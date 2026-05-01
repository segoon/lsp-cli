This is a work-in-progress. Expect some glitches.


# LSP-cli overview

LSP-cli is a simple tool allowing one to work with LSP servers from the terminal with minimal configuration.

It is able to access LSP server out of the box with zero configuration.
lsp-cli does the following:
- locates files in the selected directories
- detects filetypes of the located files
- selects LSP server responsible for the filetype handling
- downloads LSP server, if it is not available in `$PATH`
- assembles LSP server cmdline
- starts LSP server

After the LSP server is up, lsp-cli can:
- make simple queries
  * find symbol
  * show definition/prototype
  * show function body
  * list callers
  * list callees
  * start indexing
- start/stop LSP server
- proxy requests to LSP server (to abstract from LSP server implementation)

lsp-cli is configurable.
You may configure:
- filetype/language detection
- LSP server selection for a specific language
- LSP server cmdline (e.g. `-jN --background-index` for clangd)

# Quick start

```sh
git clone blablabla
cargo run
```


# Playground

The repository includes small multi-file sample projects under `playground/` for manual
`lsp-cli` experiments across multiple languages:

- `playground/c`
- `playground/cpp`
- `playground/java`
- `playground/python`
- `playground/csharp`
- `playground/rust`
- `playground/js`
- `playground/typescript`
- `playground/go`

Each playground contains cross-file references, a subdirectory, and a minimal project marker so
you can try commands such as:

```sh
cargo run -- detect playground/python
cargo run -- grep Order playground/rust
cargo run -- list-symbols playground/java/src/main/java/playground/order/Order.java
cargo run -- definition format_order playground/c --lsp clangd
```

See `playground/README.md` for more examples.


# Available modes

```sh
# Suggest LSP server cmdline based on filenames
lsp-cli detect path/to/project
lsp-cli detect path/to/project/main.py --json

# Query workspace symbols through the first detected LSP server
lsp-cli grep MySymbol path/to/project
lsp-cli grep MySymbol path/to/project --lsp clangd
lsp-cli grep --json MySymbol path/to/project
lsp-cli grep --debug MySymbol path/to/project
lsp-cli grep --timeout 1.5 MySymbol path/to/project
lsp-cli grep --timeout 100ms MySymbol path/to/project
lsp-cli grep --wait-for-index MySymbol path/to/project

# List all symbols in one file
lsp-cli list-symbols path/to/project/src/main.rs
lsp-cli list-symbols path/to/project/src/main.rs --json

# List only function-like workspace symbols
lsp-cli list-functions path/to/project
lsp-cli list-functions path/to/project --json

# List files handled by the selected server
lsp-cli list-files path/to/project
lsp-cli list-files path/to/project --limit 20

# Resolve symbol locations from fuzzy workspace-symbol matches
lsp-cli references MySymbol path/to/project
lsp-cli ref MySymbol path/to/project
lsp-cli definition MySymbol path/to/project
lsp-cli declaration MySymbol path/to/project
lsp-cli callers MyFunction path/to/project
lsp-cli callees MyFunction path/to/project

# Wait for an LSP server that exposes background-work progress to finish indexing
lsp-cli build-index path/to/project --lsp rust-analyzer
lsp-cli build-index path/to/project --lsp clangd

# Generate shell completion script to stdout
lsp-cli completion > /tmp/lsp-cli.bash
lsp-cli completion bash > /tmp/lsp-cli.bash
lsp-cli completion zsh > /tmp/_lsp-cli

# Replace lsp-cli with the detected LSP server process
lsp-cli run path/to/project --lsp rust-analyzer

# Start a background daemon and print its Unix socket path
lsp-cli daemon path/to/project --lsp rust-analyzer
```

`grep` uses the LSP `workspace/symbol` request. Pattern syntax and matching behavior are server-dependent.
`list-symbols` uses `textDocument/documentSymbol` for a single file.
`list-functions` walks matching files with `textDocument/documentSymbol` and keeps only method, constructor, function, and operator symbol kinds.
`references`, `definition`, `declaration`, `callers`, and `callees` start from `workspace/symbol` matches and then run the corresponding position-based LSP request for each match.
`--limit` defaults to `100`. Text output is limited by lines, and JSON output is limited by result items. When the limit is hit, lsp-cli prints a notice to stderr.
`--wait-for-index` waits for the same background-work signals as `build-index` before sending `workspace/symbol`.
`--debug` logs the selected LSP server command line, pid, and raw LSP traffic to stderr.
`--timeout` controls the per-request LSP timeout. Plain numbers are seconds, and values ending in `ms` are milliseconds.
`build-index` waits for background-work signals such as `experimental/serverStatus` or work-done progress. If the selected server does not expose such progress, the command fails.
`completion` writes a shell completion script to stdout. If no shell is passed, it uses the current shell from `$SHELL`.
`run` performs detection and then replaces `lsp-cli` with the detected LSP server process using `execve`.
`daemon` creates a Unix socket under `$XDG_RUNTIME_DIR/lsp-cli/`, starts `lsp-cli run` in the background, and prints the socket path only after the socket is already listening. The daemon accepts one LSP client at a time, keeps the upstream server warm while idle, and shuts it down after `--idle-timeout` (default `60s`). If the next client initializes with different normalized settings, lsp-cli restarts the upstream LSP server before serving that client.
LSP-backed commands such as `grep`, `list-symbols`, `references`, and `build-index` automatically prefer the matching daemon socket when it exists. If the socket file exists but no daemon is listening, lsp-cli removes the dead socket and falls back to starting the LSP server directly.


# References

- [LSP specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)


# Thanks

I'd like to say "thank you" to the following opensource projects:
- [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) was used to fill LSP servers database
- [mason](https://github.com/mason-org/mason.nvim) inspired me to implement LSP server autodownload


# TODO

detect:
- LSP servers priority among server for the same filetype

commands:
- repl (TODO: name... cli, console, terminal, interactive?)

options:
- -s|--signature - show the full signature
- -b|--body - show the full function body
- -l|--limit - output limit in lines
