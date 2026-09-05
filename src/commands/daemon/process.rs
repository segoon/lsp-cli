use super::events::{EventQueue, ReaderWorker, Source};
use super::writer::WriterWorker;
use super::{BACKGROUND_ENV, ClientPhase, ClientSession, Daemon, DaemonArgs, DaemonTarget};
use crate::commands::common::prepare_workspace;
use crate::config::ConfigStore;
use crate::error::{Error, Result, error_fn};
use crate::lsp::{path_to_file_uri, workspace_name};
use crate::runtime_state::{daemon_socket_path, default_daemon_root};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
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
    let mut daemon = match unsafe { setsid_wrapper() }.and_then(|()| {
        Daemon::new(
            target,
            args.server.debug,
            args.idle_timeout,
            args.write_stall_timeout,
        )
    }) {
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

impl ClientSession {
    pub(super) fn new(
        stream: UnixStream,
        events: &mut EventQueue,
        deadline: Option<Instant>,
    ) -> Result<Self> {
        let reader = stream.try_clone().map_err(|error| {
            Error::unexpected(format!("failed to clone client socket: {error}"))
        })?;

        let generation = events.next_generation()?;
        let worker = ReaderWorker::socket_with_deadline(
            reader,
            Source::Client(generation),
            events,
            deadline,
        )?;
        let writer = WriterWorker::socket(stream, Source::Client(generation), events)?;
        Ok(Self {
            writer,
            generation,
            reader: worker,
            phase: ClientPhase::WaitingForInitialize,
            wants_background_work: false,
            forwarded_client_requests: BTreeSet::new(),
            pending_server_requests: BTreeMap::new(),
            open_documents: BTreeSet::new(),
            stop_after_write: None,
            disconnect_after_write: None,
        })
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
