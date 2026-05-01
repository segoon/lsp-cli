# LSP protocol

- LSP 3.17 assumes one server serves one tool. `lsp-cli daemon` therefore implements a
  conservative proxy policy instead of transparent multi-client sharing: only one client may be
  connected at a time, downstream `shutdown`/`exit` are handled locally, and a later client with
  different normalized `initialize` settings forces a fresh upstream server.
- `lsp-cli daemon` drops stale responses for disconnected clients and closes all client-owned
  documents on disconnect, but it does not persist dynamic registrations across sessions. If the
  upstream server uses `client/registerCapability` or `client/unregisterCapability`, lsp-cli marks
  the session as non-reusable and restarts the upstream server before the next client.
- `workspace/applyEdit` only works while a client is actively connected. If no client is connected,
  lsp-cli rejects the request instead of editing files behind the client's back.

# LSP server implementations

## rust-analyzer

## clangd

- `clangd` may start successfully without sending the background-work progress notifications that
  `lsp-cli` currently expects for `wait-for-index`/`build-index` flows. Keep normal symbol-query
  configs on `wait-for-index: false` unless that progress reporting is confirmed for the target
  `clangd` setup.
