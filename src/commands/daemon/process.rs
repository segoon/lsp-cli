use super::{
    BACKGROUND_ENV, ClientPhase, ClientSession, Daemon, DaemonArgs, DaemonTarget, POLL_INTERVAL,
    ReaderEvent, UPSTREAM_SHUTDOWN_TIMEOUT, UpstreamServer,
};
use crate::commands::common::prepare_workspace;
use crate::config::ConfigStore;
use crate::error::{Error, Result, error_fn};
use crate::lsp::transport::read_message;
use crate::lsp::{jsonrpc, path_to_file_uri, workspace_name};
use crate::runtime_state::{daemon_socket_path, default_daemon_root};
use crate::server_stderr::CapturedStderr;
use crate::system_log::{
    log_lsp_server_exit, log_lsp_server_started, log_lsp_server_starting, log_unexpected_error,
};
use lsp_types::notification::{Exit, Notification};
use lsp_types::request::{Request, Shutdown};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

pub(super) fn resolve_target(args: &DaemonArgs, config: &ConfigStore) -> Result<DaemonTarget> {
    let selected = &args.server;
    let workspace = prepare_workspace(
        &args.path,
        selected.server(),
        selected.language(),
        selected.download,
        config,
    )?;
    let server = workspace.server;
    let workspace_root = fs::canonicalize(&server.workspace_root).map_err(|error| {
        Error::unexpected(format!(
            "failed to resolve {}: {error}",
            server.workspace_root.display()
        ))
    })?;
    let workspace_root_string = workspace_root.display().to_string();
    let root_uri = path_to_file_uri(&workspace_root)?;
    let workspace_name = workspace_name(&workspace_root);
    let socket_root = default_daemon_root()?;
    fs::create_dir_all(&socket_root).map_err(|error| {
        Error::unexpected(format!(
            "failed to create {}: {error}",
            socket_root.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&socket_root, permissions).map_err(|error| {
            Error::unexpected(format!(
                "failed to secure daemon socket root {}: {error}",
                socket_root.display()
            ))
        })?;
    }

    let socket_path = daemon_socket_path(
        &socket_root,
        &workspace_root,
        &server.server,
        &server.command,
    );

    Ok(DaemonTarget {
        path: args.path.clone(),
        workspace_root_string,
        root_uri,
        workspace_name,
        server_name: server.server,
        socket_path,
    })
}

pub(super) fn launch_background(args: &DaemonArgs, target: &DaemonTarget) -> Result<String> {
    launch_background_for_connection(
        &args.path,
        &target.server_name,
        &target.socket_path,
        args.server.debug,
        args.idle_timeout,
    )?;
    Ok(target.socket_path.display().to_string())
}

pub(super) fn launch_background_for_connection(
    path: &Path,
    server_name: &str,
    socket_path: &Path,
    debug: bool,
    idle_timeout: std::time::Duration,
) -> Result<()> {
    let executable = std::env::current_exe().map_err(|error| {
        Error::unexpected(format!("failed to resolve lsp-cli executable: {error}"))
    })?;
    let devnull = File::open("/dev/null")
        .map_err(error_fn!(Error::unexpected, "failed to open /dev/null"))?;
    let mut command = Command::new(executable);
    command
        .arg("daemon")
        .arg(path)
        .env(BACKGROUND_ENV, "1")
        .stdin(Stdio::from(devnull))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    command.arg("--lsp").arg(server_name);
    if debug {
        command.arg("--debug");
    }
    command
        .arg("--idle-timeout")
        .arg(idle_timeout.as_secs_f64().to_string());

    let mut child = command.spawn().map_err(error_fn!(
        Error::unexpected,
        "failed to start daemon process"
    ))?;
    let Some(stdout) = child.stdout.take() else {
        return Err(Error::unexpected("failed to capture daemon startup status"));
    };
    let mut reader = BufReader::new(stdout);
    let mut status = String::new();
    let mut payload = String::new();
    reader.read_line(&mut status).map_err(error_fn!(
        Error::unexpected,
        "failed to read daemon startup status"
    ))?;
    reader.read_line(&mut payload).map_err(error_fn!(
        Error::unexpected,
        "failed to read daemon startup payload"
    ))?;

    match status.trim_end() {
        "READY" => {
            let payload = payload.trim_end().to_string();
            if payload.is_empty() {
                return Err(Error::unexpected(
                    "daemon started without reporting a socket path",
                ));
            }
            if payload != socket_path.display().to_string() {
                return Err(Error::unexpected(format!(
                    "daemon reported unexpected socket path {payload:?}, expected {}",
                    socket_path.display()
                )));
            }
            Ok(())
        }
        "ERROR" => Err(Error::unexpected(payload.trim_end().to_string())),
        other => Err(Error::unexpected(format!(
            "unexpected daemon startup status {other:?}"
        ))),
    }
}

pub(super) fn run_background(args: &DaemonArgs, target: DaemonTarget) -> Result<String> {
    let mut daemon = match unsafe { setsid_wrapper() }
        .and_then(|()| Daemon::new(target, args.server.debug, args.idle_timeout))
    {
        Ok(daemon) => daemon,
        Err(error) => {
            let startup_error = error.to_string();
            let _ = print_startup_status("ERROR", &startup_error);
            return Err(error);
        }
    };
    print_startup_status("READY", &daemon.target.socket_path.display().to_string())?;
    daemon.serve()?;
    Ok(String::new())
}

fn print_startup_status(status: &str, payload: &str) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{status}").map_err(error_fn!(
        Error::unexpected,
        "failed to report daemon status"
    ))?;
    writeln!(stdout, "{payload}")
        .and_then(|()| stdout.flush())
        .map_err(error_fn!(
            Error::unexpected,
            "failed to flush daemon status"
        ))
}

pub(super) fn bind_listener(socket_path: &Path) -> Result<UnixListener> {
    if socket_path.exists() {
        match UnixStream::connect(socket_path) {
            Ok(_) => {
                return Err(Error::unexpected(format!(
                    "a daemon is already listening on {}",
                    socket_path.display()
                )));
            }
            Err(_) => {
                fs::remove_file(socket_path).map_err(|error| {
                    Error::unexpected(format!(
                        "failed to remove stale socket {}: {error}",
                        socket_path.display()
                    ))
                })?;
            }
        }
    }

    UnixListener::bind(socket_path).map_err(|error| {
        Error::unexpected(format!(
            "failed to bind daemon socket {}: {error}",
            socket_path.display()
        ))
    })
}

pub(super) fn spawn_reader<R>(reader: R) -> Receiver<ReaderEvent>
where
    R: std::io::Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    if sender.send(ReaderEvent::Message(message)).is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = sender.send(ReaderEvent::EndOfStream);
                    return;
                }
                Err(error) => {
                    let _ = sender.send(ReaderEvent::Error(error.to_string()));
                    return;
                }
            }
        }
    });
    receiver
}

impl ClientSession {
    pub(super) fn new(stream: UnixStream) -> Result<Self> {
        let reader = stream.try_clone().map_err(|error| {
            Error::unexpected(format!("failed to clone client socket: {error}"))
        })?;

        Ok(Self {
            writer: stream,
            messages: spawn_reader(reader),
            phase: ClientPhase::WaitingForInitialize,
            wants_background_work: false,
            forwarded_client_requests: BTreeSet::new(),
            pending_server_requests: BTreeMap::new(),
            open_documents: BTreeSet::new(),
        })
    }
}

impl UpstreamServer {
    pub(super) fn spawn(target: &DaemonTarget, debug: bool) -> Result<Self> {
        let executable = std::env::current_exe().map_err(|error| {
            Error::unexpected(format!("failed to resolve lsp-cli executable: {error}"))
        })?;
        let mut command = Command::new(executable);
        command
            .arg("run")
            .arg(&target.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if debug {
            command.arg("--debug");
        }

        command.arg("--lsp").arg(&target.server_name);

        log_lsp_server_starting();
        let mut child = command.spawn().map_err(|error| {
            let error = format!(
                "failed to start lsp-cli run for {}: {error}",
                target.server_name
            );
            log_unexpected_error(&error);
            Error::unexpected(error)
        })?;
        log_lsp_server_started(child.id());
        let Some(stdin) = child.stdin.take() else {
            let error = "failed to open LSP server stdin".to_string();
            log_unexpected_error(&error);
            return Err(Error::unexpected(error));
        };
        let Some(stdout) = child.stdout.take() else {
            let error = "failed to open LSP server stdout".to_string();
            log_unexpected_error(&error);
            return Err(Error::unexpected(error));
        };
        let stderr = CapturedStderr::spawn(
            child.stderr.take().ok_or_else(|| {
                let error = "failed to open LSP server stderr".to_string();
                log_unexpected_error(&error);
                Error::unexpected(error)
            })?,
            debug,
        );

        Ok(Self {
            child,
            stdin,
            stderr,
            messages: spawn_reader(stdout),
            initialize_fingerprint: None,
            initialize_result: None,
            restart_required: false,
            background_work: super::BackgroundWorkTracker::default(),
        })
    }

    pub(super) fn shutdown(&mut self, debug: bool) -> Result<()> {
        let _ = self.stderr.summary();
        if self.initialize_fingerprint.is_some() {
            let shutdown_id = Value::String("lsp-cli/daemon-shutdown".to_string());
            let shutdown = jsonrpc(Some(shutdown_id.clone()), Shutdown::METHOD, &())?;
            crate::lsp::transport::log_debug_message(debug, "daemon upstream <- ", &shutdown);
            let _ = crate::lsp::transport::write_message(&mut self.stdin, &shutdown);

            let started = Instant::now();
            while started.elapsed() < UPSTREAM_SHUTDOWN_TIMEOUT {
                let Some(remaining) = UPSTREAM_SHUTDOWN_TIMEOUT.checked_sub(started.elapsed())
                else {
                    break;
                };

                match self.messages.recv_timeout(remaining) {
                    Ok(ReaderEvent::Message(message)) => {
                        if super::response_id(&message).as_ref().is_some_and(|value| {
                            *value == Value::String("lsp-cli/daemon-shutdown".to_string())
                        }) {
                            break;
                        }
                    }
                    Ok(ReaderEvent::EndOfStream | ReaderEvent::Error(_)) | Err(_) => break,
                }
            }

            let exit = jsonrpc::<u64, _>(None, Exit::METHOD, &())?;
            crate::lsp::transport::log_debug_message(debug, "daemon upstream <- ", &exit);
            let _ = crate::lsp::transport::write_message(&mut self.stdin, &exit);
        }

        match self.child.try_wait() {
            Ok(Some(status)) => {
                log_lsp_server_exit(status);
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                let error = format!("failed to inspect LSP server process: {error}");
                log_unexpected_error(&error);
                return Err(Error::unexpected(error));
            }
        }

        let started = Instant::now();
        while started.elapsed() < UPSTREAM_SHUTDOWN_TIMEOUT {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    log_lsp_server_exit(status);
                    return Ok(());
                }
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(error) => {
                    let error = format!("failed to wait for LSP server exit: {error}");
                    log_unexpected_error(&error);
                    return Err(Error::unexpected(error));
                }
            }
        }

        self.child.kill().map_err(|error| {
            let error = format!("failed to stop LSP server process: {error}");
            log_unexpected_error(&error);
            Error::unexpected(error)
        })?;
        let status = self.child.wait().map_err(|error| {
            let error = format!("failed to reap LSP server process: {error}");
            log_unexpected_error(&error);
            Error::unexpected(error)
        })?;
        log_lsp_server_exit(status);
        Ok(())
    }
}

unsafe fn setsid_wrapper() -> Result<()> {
    unsafe extern "C" {
        fn setsid() -> i32;
    }

    if unsafe { setsid() } == -1 {
        return Err(Error::unexpected(format!(
            "failed to detach daemon from terminal: {}",
            std::io::Error::last_os_error()
        )));
    }

    Ok(())
}
