use crate::cli::DaemonArgs;
use crate::commands::common::{analyze_path, resolve_server};
use crate::config::ConfigStore;
use crate::lsp::transport::{log_debug_message, read_message, write_message};
use crate::lsp::{path_to_file_uri, workspace_name};
use crate::runtime_state::{daemon_socket_path, default_daemon_root};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const BACKGROUND_ENV: &str = "LSP_CLI_DAEMON_BACKGROUND";
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const BUSY_CLIENT_TIMEOUT: Duration = Duration::from_millis(250);
const UPSTREAM_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const SERVER_NOT_INITIALIZED: i64 = -32002;
const INVALID_REQUEST: i64 = -32600;
const REQUEST_CANCELLED: i64 = -32800;

pub(super) fn run(args: &DaemonArgs, config: &ConfigStore) -> Result<String, String> {
    let target = resolve_target(args, config)?;

    if std::env::var_os(BACKGROUND_ENV).is_some() {
        return run_background(args, target);
    }

    launch_background(args, &target)
}

struct DaemonTarget {
    path: PathBuf,
    workspace_root_string: String,
    root_uri: String,
    workspace_name: String,
    server_name: String,
    socket_path: PathBuf,
}

struct Daemon {
    listener: UnixListener,
    target: DaemonTarget,
    debug: bool,
    idle_timeout: Duration,
    upstream: Option<UpstreamServer>,
    active_client: Option<ClientSession>,
    orphaned_client_requests: BTreeSet<String>,
    idle_since: Instant,
}

struct UpstreamServer {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<ReaderEvent>,
    initialize_fingerprint: Option<String>,
    initialize_result: Option<Value>,
    restart_required: bool,
}

struct ClientSession {
    writer: UnixStream,
    messages: Receiver<ReaderEvent>,
    phase: ClientPhase,
    forwarded_client_requests: BTreeSet<String>,
    pending_server_requests: BTreeMap<String, Value>,
    open_documents: BTreeSet<String>,
}

enum ClientPhase {
    WaitingForInitialize,
    WaitingForInitialized { forward_to_upstream: bool },
    Ready,
    WaitingForExit,
}

enum ReaderEvent {
    Message(Value),
    EndOfStream,
    Error(String),
}

impl Daemon {
    fn new(target: DaemonTarget, debug: bool, idle_timeout: Duration) -> Result<Self, String> {
        let listener = bind_listener(&target.socket_path)?;
        listener.set_nonblocking(true).map_err(|error| {
            format!(
                "failed to set {} nonblocking: {error}",
                target.socket_path.display()
            )
        })?;
        let upstream = UpstreamServer::spawn(&target, debug)?;

        Ok(Self {
            listener,
            target,
            debug,
            idle_timeout,
            upstream: Some(upstream),
            active_client: None,
            orphaned_client_requests: BTreeSet::new(),
            idle_since: Instant::now(),
        })
    }

    fn serve(&mut self) -> Result<(), String> {
        loop {
            self.accept_connections()?;
            self.drain_upstream_messages()?;
            self.drain_client_messages()?;

            if self.active_client.is_none() && self.idle_since.elapsed() >= self.idle_timeout {
                self.shutdown_upstream()?;
                fs::remove_file(&self.target.socket_path).map_err(|error| {
                    format!(
                        "failed to remove daemon socket {}: {error}",
                        self.target.socket_path.display()
                    )
                })?;
                return Ok(());
            }

            thread::sleep(POLL_INTERVAL);
        }
    }

    fn accept_connections(&mut self) -> Result<(), String> {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if self.active_client.is_some() {
                        reject_busy_client(stream, self.debug);
                        continue;
                    }

                    self.active_client = Some(ClientSession::new(stream)?);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => {
                    return Err(format!(
                        "failed to accept client on {}: {error}",
                        self.target.socket_path.display()
                    ));
                }
            }
        }
    }

    fn drain_upstream_messages(&mut self) -> Result<(), String> {
        loop {
            let event = match self.upstream.as_ref() {
                Some(upstream) => match upstream.messages.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) => return Ok(()),
                    Err(TryRecvError::Disconnected) => {
                        self.upstream_died();
                        return Ok(());
                    }
                },
                None => return Ok(()),
            };

            match event {
                ReaderEvent::Message(message) => self.handle_upstream_message(&message)?,
                ReaderEvent::EndOfStream => {
                    self.upstream_died();
                    return Ok(());
                }
                ReaderEvent::Error(error) => {
                    self.upstream_died();
                    return Err(format!("failed to read LSP server message: {error}"));
                }
            }
        }
    }

    fn drain_client_messages(&mut self) -> Result<(), String> {
        loop {
            let event = match self.active_client.as_ref() {
                Some(client) => match client.messages.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) => return Ok(()),
                    Err(TryRecvError::Disconnected) => {
                        self.disconnect_client()?;
                        return Ok(());
                    }
                },
                None => return Ok(()),
            };

            match event {
                ReaderEvent::Message(message) => self.handle_client_message(&message)?,
                ReaderEvent::EndOfStream => {
                    self.disconnect_client()?;
                    return Ok(());
                }
                ReaderEvent::Error(error) => {
                    self.disconnect_client()?;
                    return Err(format!("failed to read daemon client message: {error}"));
                }
            }
        }
    }

    fn handle_client_message(&mut self, message: &Value) -> Result<(), String> {
        log_debug_message(self.debug, "daemon client <- ", message);
        let method = message_method(message);
        let request_id = request_id(message);
        let response_id = response_id(message);

        if let Some(response_id) = response_id {
            let Some(client) = self.active_client.as_mut() else {
                return Ok(());
            };
            let key = id_key(&response_id);
            if client.pending_server_requests.remove(&key).is_some() {
                self.write_upstream_message(message)?;
            }
            return Ok(());
        }

        match self.active_client.as_ref().map(|client| &client.phase) {
            Some(ClientPhase::WaitingForInitialize) => {
                if method == Some("initialize") && request_id.is_some() {
                    return self.handle_initialize_request(message);
                }

                if method == Some("exit") {
                    self.disconnect_client()?;
                    return Ok(());
                }

                if let Some(request_id) = request_id {
                    return self.write_client_response(&error_response(
                        &request_id,
                        SERVER_NOT_INITIALIZED,
                        "daemon client must initialize before sending requests",
                    ));
                }

                return Ok(());
            }
            Some(ClientPhase::WaitingForInitialized {
                forward_to_upstream,
            }) => {
                if method == Some("initialized") {
                    if *forward_to_upstream {
                        self.write_upstream_message(message)?;
                    }
                    if let Some(client) = self.active_client.as_mut() {
                        client.phase = ClientPhase::Ready;
                    }
                    return Ok(());
                }

                if let Some(request_id) = request_id {
                    return self.write_client_response(&error_response(
                        &request_id,
                        INVALID_REQUEST,
                        "daemon client must send initialized before other requests",
                    ));
                }

                return Ok(());
            }
            Some(ClientPhase::WaitingForExit) => {
                if method == Some("exit") {
                    self.disconnect_client()?;
                }
                return Ok(());
            }
            Some(ClientPhase::Ready) | None => {}
        }

        if method == Some("shutdown") {
            let Some(request_id) = request_id else {
                return Ok(());
            };
            if let Some(client) = self.active_client.as_mut() {
                client.phase = ClientPhase::WaitingForExit;
            }
            return self.write_client_response(&success_response(&request_id, &Value::Null));
        }

        if method == Some("exit") {
            self.disconnect_client()?;
            return Ok(());
        }

        self.track_client_document_state(method, message.get("params"));

        if let Some(request_id) = request_id {
            let Some(client) = self.active_client.as_mut() else {
                return Ok(());
            };
            client.forwarded_client_requests.insert(id_key(&request_id));
        }

        self.write_upstream_message(message)
    }

    fn handle_initialize_request(&mut self, message: &Value) -> Result<(), String> {
        let Some(request_id) = request_id(message) else {
            return Ok(());
        };
        let params = message
            .get("params")
            .cloned()
            .ok_or_else(|| "initialize request is missing params".to_string())?;
        let normalized = normalize_initialize_params(&params, &self.target)?;
        let fingerprint = fingerprint_value(&normalized);

        let should_restart = match self.upstream.as_ref() {
            Some(upstream) => {
                upstream.restart_required
                    || upstream
                        .initialize_fingerprint
                        .as_ref()
                        .is_some_and(|value| value != &fingerprint)
            }
            None => true,
        };

        if should_restart {
            self.restart_upstream()?;
        }

        if self
            .upstream
            .as_ref()
            .and_then(|upstream| upstream.initialize_fingerprint.as_ref())
            .is_some()
        {
            let result = self
                .upstream
                .as_ref()
                .and_then(|upstream| upstream.initialize_result.clone())
                .ok_or_else(|| "daemon lost cached initialize result".to_string())?;
            self.write_client_response(&success_response(&request_id, &result))?;
            if let Some(client) = self.active_client.as_mut() {
                client.phase = ClientPhase::WaitingForInitialized {
                    forward_to_upstream: false,
                };
            }
            return Ok(());
        }

        let upstream = self
            .upstream
            .as_mut()
            .ok_or_else(|| "daemon failed to start LSP server".to_string())?;
        upstream.initialize_fingerprint = Some(fingerprint);

        let forwarded = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "initialize",
            "params": normalized,
        });
        self.write_upstream_message(&forwarded)?;
        if let Some(client) = self.active_client.as_mut() {
            client.phase = ClientPhase::WaitingForInitialized {
                forward_to_upstream: true,
            };
            client.forwarded_client_requests.insert(id_key(&request_id));
        }
        Ok(())
    }

    fn handle_upstream_message(&mut self, message: &Value) -> Result<(), String> {
        log_debug_message(self.debug, "daemon upstream -> ", message);

        if let Some(response_id) = response_id(message) {
            let response_key = id_key(&response_id);

            if self.orphaned_client_requests.remove(&response_key) {
                return Ok(());
            }

            let mut forwarded_client_request = false;
            let mut initialize_response = false;
            if let Some(client) = self.active_client.as_mut() {
                forwarded_client_request = client.forwarded_client_requests.remove(&response_key);
                initialize_response = forwarded_client_request
                    && matches!(
                        client.phase,
                        ClientPhase::WaitingForInitialized {
                            forward_to_upstream: true,
                        }
                    );
                if initialize_response && message.get("error").is_some() {
                    client.phase = ClientPhase::WaitingForExit;
                }
            }

            if initialize_response && let Some(upstream) = self.upstream.as_mut() {
                if message.get("error").is_some() {
                    upstream.initialize_fingerprint = None;
                    upstream.initialize_result = None;
                    upstream.restart_required = true;
                } else {
                    upstream.initialize_result = message.get("result").cloned();
                }
            }

            if forwarded_client_request {
                return self.write_client_response(message);
            }

            return Ok(());
        }

        if let Some(request_id) = request_id(message) {
            let Some(method) = message_method(message) else {
                return Err("server request missing method".to_string());
            };

            if matches!(
                method,
                "client/registerCapability" | "client/unregisterCapability"
            ) && let Some(upstream) = self.upstream.as_mut()
            {
                upstream.restart_required = true;
            }

            if let Some(client) = self.active_client.as_mut() {
                client
                    .pending_server_requests
                    .insert(id_key(&request_id), request_id.clone());
                return self.write_client_response(message);
            }

            let response = local_server_request_response(&request_id, method);
            return self.write_upstream_message(&response);
        }

        if self.active_client.is_some() {
            return self.write_client_response(message);
        }

        Ok(())
    }

    fn track_client_document_state(&mut self, method: Option<&str>, params: Option<&Value>) {
        let Some(client) = self.active_client.as_mut() else {
            return;
        };

        match method {
            Some("textDocument/didOpen") => {
                if let Some(uri) = params
                    .and_then(|value| value.get("textDocument"))
                    .and_then(|value| value.get("uri"))
                    .and_then(Value::as_str)
                {
                    client.open_documents.insert(uri.to_string());
                }
            }
            Some("textDocument/didClose") => {
                if let Some(uri) = params
                    .and_then(|value| value.get("textDocument"))
                    .and_then(|value| value.get("uri"))
                    .and_then(Value::as_str)
                {
                    client.open_documents.remove(uri);
                }
            }
            _ => {}
        }
    }

    fn disconnect_client(&mut self) -> Result<(), String> {
        let Some(client) = self.active_client.take() else {
            return Ok(());
        };

        for uri in client.open_documents {
            let close = json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didClose",
                "params": {
                    "textDocument": {
                        "uri": uri,
                    }
                }
            });
            let _ = self.write_upstream_message(&close);
        }

        for request_key in client.forwarded_client_requests {
            let cancel = json!({
                "jsonrpc": "2.0",
                "method": "$/cancelRequest",
                "params": {
                    "id": request_id_from_key(&request_key),
                }
            });
            let _ = self.write_upstream_message(&cancel);
            self.orphaned_client_requests.insert(request_key);
        }

        for request_id in client.pending_server_requests.into_values() {
            let response = error_response(
                &request_id,
                REQUEST_CANCELLED,
                "daemon client disconnected before replying to the LSP server",
            );
            let _ = self.write_upstream_message(&response);
        }

        if self
            .upstream
            .as_ref()
            .is_some_and(|upstream| upstream.restart_required)
        {
            self.shutdown_upstream()?;
            self.upstream = Some(UpstreamServer::spawn(&self.target, self.debug)?);
        }

        self.idle_since = Instant::now();
        Ok(())
    }

    fn write_client_response(&mut self, message: &Value) -> Result<(), String> {
        log_debug_message(self.debug, "daemon client -> ", message);
        let Some(client) = self.active_client.as_mut() else {
            return Ok(());
        };
        write_message(&mut client.writer, message)
            .map_err(|error| format!("failed to write daemon client message: {error}"))
    }

    fn write_upstream_message(&mut self, message: &Value) -> Result<(), String> {
        let Some(upstream) = self.upstream.as_mut() else {
            return Err("LSP server is not running".to_string());
        };
        log_debug_message(self.debug, "daemon upstream <- ", message);
        write_message(&mut upstream.stdin, message)
            .map_err(|error| format!("failed to write LSP server message: {error}"))
    }

    fn shutdown_upstream(&mut self) -> Result<(), String> {
        if let Some(mut upstream) = self.upstream.take() {
            upstream.shutdown(self.debug)?;
        }
        self.orphaned_client_requests.clear();
        Ok(())
    }

    fn restart_upstream(&mut self) -> Result<(), String> {
        self.shutdown_upstream()?;
        self.upstream = Some(UpstreamServer::spawn(&self.target, self.debug)?);
        Ok(())
    }

    fn upstream_died(&mut self) {
        self.upstream = None;
        self.active_client = None;
        self.orphaned_client_requests.clear();
        self.idle_since = Instant::now();
    }
}

impl ClientSession {
    fn new(stream: UnixStream) -> Result<Self, String> {
        let reader = stream
            .try_clone()
            .map_err(|error| format!("failed to clone client socket: {error}"))?;

        Ok(Self {
            writer: stream,
            messages: spawn_reader(reader),
            phase: ClientPhase::WaitingForInitialize,
            forwarded_client_requests: BTreeSet::new(),
            pending_server_requests: BTreeMap::new(),
            open_documents: BTreeSet::new(),
        })
    }
}

impl UpstreamServer {
    fn spawn(target: &DaemonTarget, debug: bool) -> Result<Self, String> {
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
        })
    }

    fn shutdown(&mut self, debug: bool) -> Result<(), String> {
        if self.initialize_fingerprint.is_some() {
            let shutdown_id = Value::String("lsp-cli/daemon-shutdown".to_string());
            let shutdown = json!({
                "jsonrpc": "2.0",
                "id": shutdown_id,
                "method": "shutdown",
                "params": Value::Null,
            });
            log_debug_message(debug, "daemon upstream <- ", &shutdown);
            let _ = write_message(&mut self.stdin, &shutdown);

            let started = Instant::now();
            while started.elapsed() < UPSTREAM_SHUTDOWN_TIMEOUT {
                let Some(remaining) = UPSTREAM_SHUTDOWN_TIMEOUT.checked_sub(started.elapsed())
                else {
                    break;
                };

                match self.messages.recv_timeout(remaining) {
                    Ok(ReaderEvent::Message(message)) => {
                        if response_id(&message).as_ref().is_some_and(|value| {
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
            log_debug_message(debug, "daemon upstream <- ", &exit);
            let _ = write_message(&mut self.stdin, &exit);
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

fn resolve_target(args: &DaemonArgs, config: &ConfigStore) -> Result<DaemonTarget, String> {
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

fn launch_background(args: &DaemonArgs, target: &DaemonTarget) -> Result<String, String> {
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

fn run_background(args: &DaemonArgs, target: DaemonTarget) -> Result<String, String> {
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

fn bind_listener(socket_path: &Path) -> Result<UnixListener, String> {
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

fn spawn_reader<R>(reader: R) -> Receiver<ReaderEvent>
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

fn reject_busy_client(mut stream: UnixStream, debug: bool) {
    let _ = stream.set_read_timeout(Some(BUSY_CLIENT_TIMEOUT));
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let Ok(Some(message)) = read_message(&mut reader) else {
        return;
    };
    log_debug_message(debug, "daemon busy client <- ", &message);

    let Some(request_id) = request_id(&message) else {
        return;
    };
    if message_method(&message) != Some("initialize") {
        return;
    }

    let response = error_response(
        &request_id,
        REQUEST_CANCELLED,
        "another daemon client is already connected",
    );
    let _ = write_message(&mut stream, &response);
}

fn local_server_request_response(request_id: &Value, method: &str) -> Value {
    match method {
        "window/showMessageRequest"
        | "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create" => success_response(request_id, &Value::Null),
        "workspace/configuration" | "workspace/workspaceFolders" => {
            success_response(request_id, &json!([]))
        }
        "workspace/applyEdit" => json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "applied": false,
                "failureReason": "no daemon client is connected to apply workspace edits",
            }
        }),
        _ => error_response(
            request_id,
            INVALID_REQUEST,
            &format!("daemon does not support server request {method}"),
        ),
    }
}

fn normalize_initialize_params(params: &Value, target: &DaemonTarget) -> Result<Value, String> {
    let Some(object) = params.as_object() else {
        return Err("initialize params must be a JSON object".to_string());
    };

    if let Some(root_uri) = object.get("rootUri")
        && !root_uri.is_null()
        && root_uri.as_str() != Some(target.root_uri.as_str())
    {
        return Err(format!(
            "daemon client rootUri must match {}",
            target.root_uri
        ));
    }

    if let Some(root_path) = object.get("rootPath")
        && !root_path.is_null()
        && root_path.as_str() != Some(target.workspace_root_string.as_str())
    {
        return Err(format!(
            "daemon client rootPath must match {}",
            target.workspace_root_string
        ));
    }

    if let Some(workspace_folders) = object.get("workspaceFolders")
        && !workspace_folders.is_null()
    {
        let Some(items) = workspace_folders.as_array() else {
            return Err("initialize workspaceFolders must be an array or null".to_string());
        };
        if items.len() != 1
            || items[0].get("uri").and_then(Value::as_str) != Some(target.root_uri.as_str())
        {
            return Err(format!(
                "daemon client workspaceFolders must contain only {}",
                target.root_uri
            ));
        }
    }

    let mut normalized = object.clone();
    normalized.insert(
        "processId".to_string(),
        Value::from(u64::from(std::process::id())),
    );
    normalized.remove("workDoneToken");
    normalized.insert(
        "rootUri".to_string(),
        Value::String(target.root_uri.clone()),
    );
    normalized.insert(
        "rootPath".to_string(),
        Value::String(target.workspace_root_string.clone()),
    );
    normalized.insert(
        "workspaceFolders".to_string(),
        json!([{
            "uri": target.root_uri,
            "name": target.workspace_name,
        }]),
    );

    Ok(Value::Object(normalized))
}

fn request_id(message: &Value) -> Option<Value> {
    message
        .get("id")
        .filter(|_| message.get("method").is_some())
        .cloned()
}

fn response_id(message: &Value) -> Option<Value> {
    message
        .get("id")
        .filter(|_| message.get("method").is_none())
        .cloned()
}

fn message_method(message: &Value) -> Option<&str> {
    message.get("method").and_then(Value::as_str)
}

fn success_response(id: &Value, result: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

fn fingerprint_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(string) => {
            serde_json::to_string(string).unwrap_or_else(|_| "\"<invalid-string>\"".to_string())
        }
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(fingerprint_value)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(object) => {
            let mut pairs = object.iter().collect::<Vec<_>>();
            pairs.sort_by(|(left, _), (right, _)| left.cmp(right));
            format!(
                "{{{}}}",
                pairs
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key)
                            .unwrap_or_else(|_| "\"<invalid-key>\"".to_string()),
                        fingerprint_value(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

fn id_key(id: &Value) -> String {
    fingerprint_value(id)
}

fn request_id_from_key(key: &str) -> Value {
    serde_json::from_str(key).unwrap_or_else(|_| Value::String(key.to_string()))
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

#[cfg(test)]
mod tests {
    use super::{fingerprint_value, normalize_initialize_params};
    use crate::runtime_state::daemon_socket_path;
    use crate::test_support::TestDir;
    use serde_json::json;

    fn daemon_target(dir: &TestDir) -> super::DaemonTarget {
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace should exist");

        super::DaemonTarget {
            path: workspace_root.clone(),
            workspace_root_string: workspace_root.display().to_string(),
            root_uri: crate::lsp::path_to_file_uri(&workspace_root).expect("uri should build"),
            workspace_name: crate::lsp::workspace_name(&workspace_root),
            server_name: "rust-analyzer".to_string(),
            socket_path: dir.path().join("daemon.sock"),
        }
    }

    #[test]
    fn socket_path_changes_with_workspace_and_command() {
        let dir = TestDir::new("daemon-socket");
        let socket_root = dir.path().join("run");
        let first = daemon_socket_path(
            &socket_root,
            &dir.path().join("one"),
            "rust-analyzer",
            &["rust-analyzer".to_string()],
        );
        let second = daemon_socket_path(
            &socket_root,
            &dir.path().join("two"),
            "rust-analyzer",
            &["rust-analyzer".to_string()],
        );
        let third = daemon_socket_path(
            &socket_root,
            &dir.path().join("one"),
            "rust-analyzer",
            &["rust-analyzer".to_string(), "--stdio".to_string()],
        );

        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn normalize_initialize_params_rewrites_process_id_and_workspace() {
        let dir = TestDir::new("daemon-normalize");
        let target = daemon_target(&dir);
        let params = json!({
            "processId": 1,
            "rootUri": target.root_uri,
            "rootPath": target.workspace_root_string,
            "workspaceFolders": [{"uri": target.root_uri, "name": "ignored"}],
            "workDoneToken": "abc",
            "capabilities": {"workspace": {"configuration": true}},
        });

        let normalized =
            normalize_initialize_params(&params, &target).expect("params should normalize");

        assert_eq!(
            normalized
                .get("rootUri")
                .and_then(serde_json::Value::as_str),
            Some(target.root_uri.as_str())
        );
        assert_eq!(
            normalized
                .get("rootPath")
                .and_then(serde_json::Value::as_str),
            Some(target.workspace_root_string.as_str())
        );
        assert!(normalized.get("workDoneToken").is_none());
    }

    #[test]
    fn normalize_initialize_params_rejects_other_workspace() {
        let dir = TestDir::new("daemon-normalize");
        let target = daemon_target(&dir);
        let error = normalize_initialize_params(
            &json!({
                "rootUri": "file:///elsewhere",
            }),
            &target,
        )
        .expect_err("mismatched workspace should fail");

        assert!(error.contains("rootUri"));
    }

    #[test]
    fn fingerprint_value_sorts_object_keys() {
        let left = json!({"b": 1, "a": [true, null]});
        let right = json!({"a": [true, null], "b": 1});

        assert_eq!(fingerprint_value(&left), fingerprint_value(&right));
    }
}
