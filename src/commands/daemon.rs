use crate::cli::DaemonArgs;
use crate::config::ConfigStore;
use crate::lsp::{STOP_METHOD, jsonrpc, parse_lsp_uri};
use crate::lsp::transport::{log_debug_message, write_message};
use lsp_types::notification::{Cancel, DidCloseTextDocument, Notification};
use lsp_types::request::{Initialize, Request};
use lsp_types::{CancelParams, DidCloseTextDocumentParams, NumberOrString, TextDocumentIdentifier};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, ChildStdin};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

mod process;
mod protocol;

#[cfg(test)]
mod tests;

use process::{bind_listener, launch_background, resolve_target, run_background};
use protocol::{
    BackgroundWorkTracker, ReaderEvent, error_response, fingerprint_value,
    handle_busy_connection, id_key, local_server_request_response, message_method,
    normalize_initialize_params, read_control_message, request_id, request_id_from_key,
    respond_to_stop_request, response_id, stop_request, stop_request_id, success_response,
    update_background_work_tracker, wants_background_work,
};

const BACKGROUND_ENV: &str = "LSP_CLI_DAEMON_BACKGROUND";
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const BUSY_CLIENT_TIMEOUT: Duration = Duration::from_millis(250);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);
const DETACHED_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);
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

pub(super) fn launch_for_workspace(
    workspace_root: &Path,
    server_name: &str,
    socket_path: &Path,
    debug: bool,
) -> Result<(), String> {
    process::launch_background_for_connection(
        workspace_root,
        server_name,
        socket_path,
        debug,
        DETACHED_IDLE_TIMEOUT,
    )
}

pub(super) enum StopSocketResult {
    Stopped,
    RemovedStaleSocket,
    NotRunning,
}

pub(super) fn stop_socket(socket_path: &Path, debug: bool) -> Result<StopSocketResult, String> {
    if !socket_path.exists() {
        return Ok(StopSocketResult::NotRunning);
    }

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(stream) => stream,
        Err(connect_error) => match fs::remove_file(socket_path) {
            Ok(()) => return Ok(StopSocketResult::RemovedStaleSocket),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(StopSocketResult::NotRunning);
            }
            Err(error) => {
                return Err(format!(
                    "failed to connect to daemon socket {}: {connect_error}; failed to remove stale socket: {error}",
                    socket_path.display()
                ));
            }
        },
    };

    let request = stop_request();
    log_debug_message(debug, "daemon control <- ", &request);
    write_message(&mut stream, &request)
        .map_err(|error| format!("failed to write daemon stop request: {error}"))?;
    let response = read_control_message(&stream, CONTROL_TIMEOUT, debug)?
        .ok_or_else(|| "daemon closed the stop control socket without replying".to_string())?;

    if response_id(&response) != stop_request_id(&request) {
        return Err("daemon returned an unexpected stop response id".to_string());
    }
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown daemon stop error");
        return Err(message.to_string());
    }

    wait_for_stopped_socket(socket_path)?;

    Ok(StopSocketResult::Stopped)
}

fn wait_for_stopped_socket(socket_path: &Path) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < STOP_COMPLETION_TIMEOUT {
        if !socket_path.exists() {
            return Ok(());
        }

        match UnixStream::connect(socket_path) {
            Ok(_) => thread::sleep(POLL_INTERVAL),
            Err(_) => match fs::remove_file(socket_path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(format!(
                        "daemon stopped listening on {} but its socket could not be removed: {error}",
                        socket_path.display()
                    ));
                }
            },
        }
    }

    Err(format!(
        "daemon acknowledged stop on {} but did not exit before the timeout",
        socket_path.display()
    ))
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
    stop_requested: bool,
}

struct UpstreamServer {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<ReaderEvent>,
    initialize_fingerprint: Option<String>,
    initialize_result: Option<Value>,
    restart_required: bool,
    background_work: BackgroundWorkTracker,
}

struct ClientSession {
    writer: UnixStream,
    messages: Receiver<ReaderEvent>,
    phase: ClientPhase,
    wants_background_work: bool,
    forwarded_client_requests: BTreeSet<String>,
    pending_server_requests: BTreeMap<String, Value>,
    open_documents: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum ClientPhase {
    WaitingForInitialize,
    WaitingForInitialized { forward_to_upstream: bool },
    Ready,
    WaitingForExit,
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
            stop_requested: false,
        })
    }

    fn serve(&mut self) -> Result<(), String> {
        loop {
            self.accept_connections()?;
            self.drain_upstream_messages()?;
            self.drain_client_messages()?;

            if self.stop_requested {
                return self.stop();
            }

            if self.active_client.is_none() && self.idle_since.elapsed() >= self.idle_timeout {
                return self.stop();
            }

            thread::sleep(POLL_INTERVAL);
        }
    }

    fn accept_connections(&mut self) -> Result<(), String> {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if self.active_client.is_some() {
                        if handle_busy_connection(stream, self.debug)? {
                            self.stop_requested = true;
                        }
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

        match self.active_client.as_ref().map(|client| client.phase) {
            Some(ClientPhase::WaitingForInitialize) => {
                if stop_request_id(message).is_some() {
                    return self.handle_stop_request(message);
                }

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
                    if forward_to_upstream {
                        self.write_upstream_message(message)?;
                    }
                    if let Some(client) = self.active_client.as_mut() {
                        client.phase = ClientPhase::Ready;
                    }
                    self.notify_client_if_background_ready(!forward_to_upstream)?;
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

        if method == Some(STOP_METHOD) {
            return self.handle_stop_request(message);
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
        let wants_background_work = wants_background_work(&normalized);

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
                client.wants_background_work = wants_background_work;
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

        let forwarded = jsonrpc(Some(request_id.clone()), Initialize::METHOD, &normalized)?;
        self.write_upstream_message(&forwarded)?;
        if let Some(client) = self.active_client.as_mut() {
            client.wants_background_work = wants_background_work;
            client.phase = ClientPhase::WaitingForInitialized {
                forward_to_upstream: true,
            };
            client.forwarded_client_requests.insert(id_key(&request_id));
        }
        Ok(())
    }

    fn handle_stop_request(&mut self, message: &Value) -> Result<(), String> {
        let Some(client) = self.active_client.as_mut() else {
            return Ok(());
        };

        respond_to_stop_request(&mut client.writer, message, self.debug)?;
        self.stop_requested = true;
        Ok(())
    }

    fn handle_upstream_message(&mut self, message: &Value) -> Result<(), String> {
        log_debug_message(self.debug, "daemon upstream -> ", message);

        if let Some(upstream) = self.upstream.as_mut() {
            update_background_work_tracker(message, &mut upstream.background_work)?;
        }

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
            let params = DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier::new(parse_lsp_uri(&uri, "document")?),
            };
            let close = jsonrpc::<u64, _>(None, DidCloseTextDocument::METHOD, &params)?;
            let _ = self.write_upstream_message(&close);
        }

        for request_key in client.forwarded_client_requests {
            let id = serde_json::from_value::<NumberOrString>(request_id_from_key(&request_key))
                .map_err(|error| format!("invalid cancel request id: {error}"))?;
            let params = CancelParams {
                id,
            };
            let cancel = jsonrpc::<u64, _>(None, Cancel::METHOD, &params)?;
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

        if !self.stop_requested
            && self
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

    fn stop(&mut self) -> Result<(), String> {
        self.disconnect_client()?;
        self.shutdown_upstream()?;
        match fs::remove_file(&self.target.socket_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "failed to remove daemon socket {}: {error}",
                self.target.socket_path.display()
            )),
        }
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
