# LSP protocol

## Capabilities

- Some servers send client requests such as `client/registerCapability` immediately after the
  `initialize` response and expect those requests to be answered before later client traffic.
  lsp-cli therefore drains and replies to queued server requests right after `initialized` and
  before sending later requests, instead of assuming request-response traffic is strictly
  one-directional.


## Diagnostics

- Diagnostics are not uniformly query-shaped across servers.
  Some servers support client-initiated `textDocument/diagnostic`, while others only publish
  `textDocument/publishDiagnostics` asynchronously after `didOpen` or background analysis.
  A CLI diagnostics command therefore cannot rely on only one path if it wants broad coverage.
- `textDocument/publishDiagnostics` is latest-state data, not an append-only stream.
  Servers may replace older diagnostics for the same URI with a newer notification, including an
  empty list to clear prior errors. Keep only the latest notification per URI instead of treating
  each publish as an independent result item.
- A diagnostics command that only opens files and waits a short fixed delay is fragile.
  Some servers publish diagnostics only after preamble building, indexing, or other async work, so
  the client may need to wait through a bounded timeout budget instead of assuming diagnostics are
  available immediately after `didOpen`.
- Background-work progress such as `$/progress` is useful for diagnostics timing, but it is not a
  diagnostics result by itself. A server can report indexing activity without publishing any
  diagnostics yet, and some servers expose progress inconsistently, so progress should be treated as
  a hint rather than as the sole completion signal for `diag`.


# Daemon gotchas

- LSP 3.17 assumes one server serves one tool. `lsp-cli daemon` therefore implements a
  conservative proxy policy instead of transparent multi-client sharing: only one client may be
  connected at a time, downstream `shutdown`/`exit` are handled locally, and a later client with
  different normalized `initialize` settings forces a fresh upstream server.
- `lsp-cli daemon` drops stale responses for disconnected clients and closes all client-owned
  documents on disconnect, but it does not persist dynamic registrations across sessions. If the
  upstream server uses `client/registerCapability` or `client/unregisterCapability`, lsp-cli marks
  the session as non-reusable and restarts the upstream server before the next client.
- Reused daemon sessions can attach after the upstream server has already finished indexing. To
  keep `wait-for-index` semantics stable for warm sessions, the daemon synthesizes a quiescent
  `experimental/serverStatus` notification after cached `initialize` replies when it already knows
  the upstream server is idle.
- Normal LSP commands opportunistically reuse the daemon socket. If the expected socket path exists
  but no daemon listens on it anymore, lsp-cli treats it as stale runtime state, removes the dead
  socket file, and falls back to starting a direct LSP server for that command.
- `stop` and `stop-all` use a private daemon control request over the Unix socket instead of normal
  LSP `shutdown`. This keeps regular client shutdown local to the proxy, but it also means `stop`
  only finds daemons whose socket path still matches the currently resolved workspace root and LSP
  command line; use `stop-all` when config changes make the exact match ambiguous.
- `workspace/applyEdit` only works while a client is actively connected. If no client is connected,
  lsp-cli rejects the request instead of editing files behind the client's back.
- During the LuaLS request-window benchmark, one `stop-all` cleanup reported an unexpected
  stop-response ID even though the daemon exited. Retrying against the same isolated runtime
  directory removed its stale socket. The unexpected response was not diagnosed; a failed stop
  response does not necessarily mean the daemon remains alive.
- Release profiling also reproduced a daemon exit after a successful query with
  `failed to write daemon client message: failed to write JSON-RPC message: Broken pipe`.
  Upstream notifications can race with client disconnect: the coordinator drains upstream
  traffic before client events, and a failed downstream write propagates out of `serve`.
  This can leave a stale socket and lose the indexed server between commands. Repeated commands
  must not be described as warm measurements without verifying process reuse. A regression
  test for a fix should close the client while an upstream notification is queued and verify
  that the daemon remains available to a subsequent client.

# LSP server implementations

## kotlin-lsp

- The Mason package exposes Kotlin LSP as `intellij-server`, not `kotlin-lsp`. The data config must
  use the packaged launcher name so `--download` can resolve it; a display name or upstream product
  name is not necessarily an executable name.

## roslyn-language-server

- The Mason package exposes Roslyn through the `roslyn-language-server` .NET tool launcher. A data
  config containing a literal installation placeholder cannot work with generic `--download`;
  launch the Mason-exposed command and let the NuGet backend manage its concrete installation path.

## lua-language-server

- Opening every source file for symbol discovery also triggers diagnostics. The installed
  LuaLS `textDocument/didOpen` handler opens and compiles the file; its diagnostic provider
  runs diagnostics on file-open events. In a release-build `parley.nvim` experiment with 193
  Lua files and a request window of 20, an explicit temporary configuration with diagnostics
  enabled took 12.57/12.37 seconds; disabling only diagnostics took 4.50/2.84 seconds with
  identical reference matches. Traces showed zero diagnostic notifications in the disabled
  case. This is a server-specific experimental control, not a recommendation to silently
  disable diagnostics for normal commands or shared sessions.
- In a September 2026 investigation against `parley.nvim`, `workspace/symbol` returned null for
  the local function `normalize_timestamp`, while `textDocument/documentSymbol` exposed it and
  `textDocument/references` found its call site. Workspace-symbol results alone therefore cannot
  replace document-symbol discovery for this query.
- Two direct-process runs in that investigation completed their queries but failed while waiting
  for LuaLS to exit, adding the configured 30-second timeout. The debug trace showed a successful
  `shutdown` response followed by an `exit` notification. The cause of the process staying alive
  was not established; detached runs completed successfully. Do not assume switching off detach
  is a reliable performance workaround for this setup.

## rust-analyzer

- `rust-analyzer` may answer `textDocument/documentSymbol` with flat `SymbolInformation` items whose
  `location.range.start` points at the start of the whole declaration (for example `pub fn` or an
  attribute line) instead of the identifier itself. When a later LSP request needs a precise
  symbol position, prefer recovering the identifier offset from source text inside that range
  instead of assuming `range.start` is directly queryable.

## clangd

- `clangd` may start successfully without sending the background-work progress notifications that
  `lsp-cli` currently expects for `wait-for-index`/`build-index` flows. Keep normal symbol-query
  configs on `wait-for-index: false` unless that progress reporting is confirmed for the target
  `clangd` setup.
- `clangd` may expose diagnostics only through delayed `textDocument/publishDiagnostics` even when
  it does send `$/progress`, and in some setups it does not advertise `diagnosticProvider` for
  pull diagnostics at all. For `lsp-cli diag`, prefer pull diagnostics when the capability is
  advertised, but keep a timeout-bounded push fallback for `clangd`-style behavior.
