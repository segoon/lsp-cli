use super::{
    BACKGROUND_ENV, ClientPhase, ClientSession, Daemon, DaemonArgs, DaemonTarget, POLL_INTERVAL,
    ReaderEvent, UPSTREAM_SHUTDOWN_TIMEOUT, UpstreamServer,
};
use crate::commands::common::{analyze_path, resolve_server};
use crate::config::ConfigStore;
use crate::lsp::transport::read_message;
use crate::lsp::{path_to_file_uri, workspace_name};
use crate::runtime_state::{daemon_socket_path, default_daemon_root};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

pub(super) fn resolve_target(
    args: &DaemonArgs,
    config: &ConfigStore,
) -> Result<DaemonTarget, String> {
    let (detection, suggestions) = analyze_path(&args.path, config)?;
    let server = resolve_server(&detection, &suggestions, args.lsp.as_deref())?;
    let workspace_root = fs::canonicalize(&server.workspace_root).map_err(|error| {
        format!(
            "failed to resolve {}: {error}",
            server.workspace_root.display()
        )
    })?;
    let workspace_root_string = workspace_root.display().to_string();
    let root_uri = path_to_file_uri(&workspace_root)?;
    let workspace_name = workspace_name(&workspace_root);
    let socket_root = default_daemon_root()?;
    fs::create_dir_all(&socket_root)
        .map_err(|error| format!("failed to create {}: {error}", socket_root.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&socket_root, permissions).map_err(|error| {
            format!(
                "failed to secure daemon socket root {}: {error}",
                socket_root.display()
            )
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

pub(super) fn launch_background(
    args: &DaemonArgs,
    target: &DaemonTarget,
) -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to resolve lsp-cli executable: {error}"))?;
    let devnull =
        File::open("/dev/null").map_err(|error| format!("failed to open /dev/null: {error}"))?;
    let mut command = Command::new(executable);
    command
        .arg("daemon")
        .arg(&args.path)
        .env(BACKGROUND_ENV, "1")
        .stdin(Stdio::from(devnull))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    if let Some(server) = &args.lsp {
        command.arg("--lsp").arg(server);
    }
    if args.debug {
        command.arg("--debug");
    }
    command
        .arg("--idle-timeout")
        .arg(args.idle_timeout.as_secs_f64().to_string());

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start daemon process: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture daemon startup status".to_string())?;
    let mut reader = BufReader::new(stdout);
    let mut status = String::new();
    let mut payload = String::new();
    reader
        .read_line(&mut status)
        .map_err(|error| format!("failed to read daemon startup status: {error}"))?;
    reader
        .read_line(&mut payload)
        .map_err(|error| format!("failed to read daemon startup payload: {error}"))?;

    match status.trim_end() {
        "READY" => {
            let payload = payload.trim_end().to_string();
            if payload.is_empty() {
                return Err("daemon started without reporting a socket path".to_string());
            }
            if payload != target.socket_path.display().to_string() {
                return Err(format!(
                    "daemon reported unexpected socket path {payload:?}, expected {}",
                    target.socket_path.display()
                ));
            }
            Ok(payload)
        }
        "ERROR" => Err(payload.trim_end().to_string()),
        other => Err(format!("unexpected daemon startup status {other:?}")),
    }
}

pub(super) fn run_background(args: &DaemonArgs, target: DaemonTarget) -> Result<String, String> {
    let mut daemon = match unsafe { setsid_wrapper() }
        .and_then(|()| Daemon::new(target, args.debug, args.idle_timeout))
    {
        Ok(daemon) => daemon,
        Err(error) => {
            let _ = print_startup_status("ERROR", &error);
            return Err(error);
        }
    };
    print_startup_status("READY", &daemon.target.socket_path.display().to_string())?;
    daemon.serve()?;
    Ok(String::new())
}

fn print_startup_status(status: &str, payload: &str) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{status}")
        .map_err(|error| format!("failed to report daemon status: {error}"))?;
    writeln!(stdout, "{payload}")
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("failed to flush daemon status: {error}"))
}

pub(super) fn bind_listener(socket_path: &Path) -> Result<UnixListener, String> {
    if socket_path.exists() {
        match UnixStream::connect(socket_path) {
            Ok(_) => {
                return Err(format!(
                    "a daemon is already listening on {}",
                    socket_path.display()
                ));
            }
            Err(_) => {
                fs::remove_file(socket_path).map_err(|error| {
                    format!(
                        "failed to remove stale socket {}: {error}",
                        socket_path.display()
                    )
                })?;
            }
        }
    }

    UnixListener::bind(socket_path).map_err(|error| {
        format!(
            "failed to bind daemon socket {}: {error}",
            socket_path.display()
        )
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
                    let _ = sender.send(ReaderEvent::Error(error));
                    return;
                }
            }
        }
    });
    receiver
}

impl ClientSession {
    pub(super) fn new(stream: UnixStream) -> Result<Self, String> {
        let reader = stream
            .try_clone()
            .map_err(|error| format!("failed to clone client socket: {error}"))?;

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
    pub(super) fn spawn(target: &DaemonTarget, debug: bool) -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("failed to resolve lsp-cli executable: {error}"))?;
        let mut command = Command::new(executable);
        command
            .arg("run")
            .arg(&target.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if debug {
            command.arg("--debug");
        }

        command.arg("--lsp").arg(&target.server_name);

        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to start lsp-cli run for {}: {error}",
                target.server_name
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open LSP server stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to open LSP server stdout".to_string())?;

        Ok(Self {
            child,
            stdin,
            messages: spawn_reader(stdout),
            initialize_fingerprint: None,
            initialize_result: None,
            restart_required: false,
            background_work: super::BackgroundWorkTracker::default(),
        })
    }

    pub(super) fn shutdown(&mut self, debug: bool) -> Result<(), String> {
        if self.initialize_fingerprint.is_some() {
            let shutdown_id = Value::String("lsp-cli/daemon-shutdown".to_string());
            let shutdown = json!({
                "jsonrpc": "2.0",
                "id": shutdown_id,
                "method": "shutdown",
                "params": Value::Null,
            });
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

            let exit = json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": Value::Null,
            });
            crate::lsp::transport::log_debug_message(debug, "daemon upstream <- ", &exit);
            let _ = crate::lsp::transport::write_message(&mut self.stdin, &exit);
        }

        match self.child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => {
                return Err(format!("failed to inspect LSP server process: {error}"));
            }
        }

        let started = Instant::now();
        while started.elapsed() < UPSTREAM_SHUTDOWN_TIMEOUT {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(error) => {
                    return Err(format!("failed to wait for LSP server exit: {error}"));
                }
            }
        }

        self.child
            .kill()
            .map_err(|error| format!("failed to stop LSP server process: {error}"))?;
        self.child
            .wait()
            .map_err(|error| format!("failed to reap LSP server process: {error}"))?;
        Ok(())
    }
}

unsafe fn setsid_wrapper() -> Result<(), String> {
    unsafe extern "C" {
        fn setsid() -> i32;
    }

    if unsafe { setsid() } == -1 {
        return Err(format!(
            "failed to detach daemon from terminal: {}",
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
}
