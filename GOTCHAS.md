# LSP protocol

# LSP server implementations

## rust-analyzer

## clangd

- `clangd` may start successfully without sending the background-work progress notifications that
  `lsp-cli` currently expects for `wait-for-index`/`build-index` flows. Keep normal symbol-query
  configs on `wait-for-index: false` unless that progress reporting is confirmed for the target
  `clangd` setup.
