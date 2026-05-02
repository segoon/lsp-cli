use serde::{Deserialize, Serialize};

// These protocol extensions are used by specific servers or by lsp-cli itself,
// so they live next to the LSP integration even though lsp-types does not define them.
pub const SERVER_STATUS_METHOD: &str = "experimental/serverStatus";
pub const STOP_METHOD: &str = "$/lsp-cli/stop";

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct StopParams;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerStatusParams {
    pub health: String,
    pub quiescent: bool,
    pub message: Option<String>,
}
