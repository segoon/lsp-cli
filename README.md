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
git clone --recurse-submodules https://github.com/segoon/lsp-cli.git
cd lsp-cli
cargo run
```

If you already cloned the repository before `data/` became a submodule, run:

```sh
git submodule update --init --recursive
```


# Config defaults

`lsp-cli` optionally loads `lsp-cli.yaml` in this order:

1. global: `$LSP_DATA/lsp-cli.yaml` if `LSP_DATA` is set, otherwise `~/.local/share/lsp-cli/data/lsp-cli.yaml` when downloaded data exists, otherwise `data/lsp-cli.yaml` from the checked-out `data/` submodule
2. user: `$XDG_CONFIG_HOME/lsp-cli/lsp-cli.yaml`, or `~/.config/lsp-cli/lsp-cli.yaml` when `XDG_CONFIG_HOME` is unset

User settings override global settings.
Command-line flags override both.
`lsp.<language>` sets server priority for automatic selection when multiple detected servers match the same language.

Example:

```yaml
download: false
download-version: latest
detach: false
json: false
debug: false
timeout: "10"
limit: 100

detect:
  quiet: false

daemon:
  idle-timeout: "60"

lsp:
  cpp:
    - clangd
  python:
    - ty
    - pyright
    - jedi-language-server
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
lsp-cli detect path/to/project --lang python
lsp-cli detect path/to/project --lsp pyright-langserver
lsp-cli detect path/to/project/main.py --json

# Query workspace symbols through the first detected LSP server
lsp-cli grep MySymbol path/to/project
lsp-cli grep MySymbol path/to/project --lang python
lsp-cli grep MySymbol path/to/project --lsp clangd
lsp-cli grep --json MySymbol path/to/project
lsp-cli grep --debug MySymbol path/to/project
lsp-cli grep --timeout 1.5 MySymbol path/to/project
lsp-cli grep --timeout 100ms MySymbol path/to/project
lsp-cli grep --wait-for-index MySymbol path/to/project

# List symbols in one file or across one workspace directory
lsp-cli list-symbols path/to/project
lsp-cli list-symbols path/to/project/src/main.rs
lsp-cli list-symbols path/to/project --json
lsp-cli list-symbols path/to/project/src/main.rs --json

# List only function-like workspace symbols
lsp-cli list-functions path/to/project
lsp-cli list-functions path/to/project --json

# List files handled by the selected server
lsp-cli list-files path/to/project
lsp-cli list-files path/to/project --lang cpp
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

# Show the selected server command line and initialize capabilities
lsp-cli server-capabilities path/to/project
lsp-cli server-capabilities path/to/project --lsp clangd

# Generate shell completion script to stdout
lsp-cli completion > /tmp/lsp-cli.bash
lsp-cli completion bash > /tmp/lsp-cli.bash
lsp-cli completion zsh > /tmp/_lsp-cli

# Replace lsp-cli with the detected LSP server process
lsp-cli run path/to/project --lsp rust-analyzer
lsp-cli run path/to/project --lang rust

# Start a background daemon and print its Unix socket path
lsp-cli daemon path/to/project --lsp rust-analyzer
lsp-cli daemon path/to/project --lang rust

# Stop the matching daemon for one workspace/server selection
lsp-cli stop path/to/project --lsp rust-analyzer
lsp-cli stop path/to/project --lang rust

# Stop every active daemon under $XDG_RUNTIME_DIR/lsp-cli
lsp-cli stop-all

# Download or refresh the external lsp-cli data bundle
lsp-cli update

# List configured languages and servers from the loaded config root
lsp-cli languages
lsp-cli servers
lsp-cli servers --lang rust
```

`grep` uses the LSP `workspace/symbol` request. Pattern syntax and matching behavior are server-dependent.
`list-symbols` uses `textDocument/documentSymbol`. For a file input it lists that file's symbols; for a directory input it walks matching files in that workspace.
`list-functions` walks matching files with `textDocument/documentSymbol` and keeps only method, constructor, function, and operator symbol kinds.
`references`, `definition`, `declaration`, `callers`, and `callees` start from `workspace/symbol` matches and then run the corresponding position-based LSP request for each match.
`--limit` defaults to `100`. Text output is limited by lines, and JSON output is limited by result items. When the limit is hit, lsp-cli prints a notice to stderr.
`--wait-for-index` waits for the same background-work signals as `build-index` before sending `workspace/symbol`.
`--download` on LSP-spawning commands keeps non-installed detected servers in consideration and installs the selected server automatically before launch.
`--debug` logs the selected LSP server command line, pid, and raw LSP traffic to stderr.
`--timeout` controls the per-request LSP timeout. Plain numbers are seconds, and values ending in `ms` are milliseconds.
`detect --lang` and `detect --lsp` filter the detected server list to one language, one server, or their intersection.
`--lang` narrows server selection to one detected language. LSP-backed commands error on mixed-language workspaces unless you pass `--lang` or an explicit `--lsp`.
`server-capabilities` runs the normal LSP initialize handshake, prints the selected server command line and server version when available, then renders the initialize result capabilities as a YAML-like tree. Known standard capabilities use human-readable labels; unknown and experimental capabilities fall back to raw capability names. The output reflects what the server advertises during initialize, so later dynamic capability registration is not included.
`build-index` waits for background-work signals such as `experimental/serverStatus` or work-done progress. If the selected server does not expose such progress, the command fails.
`completion` writes a shell completion script to stdout. If no shell is passed, it uses the current shell from `$SHELL`.
`run` performs detection and then replaces `lsp-cli` with the detected LSP server process using `execve`.
`daemon` creates a Unix socket under `$XDG_RUNTIME_DIR/lsp-cli/`, starts `lsp-cli run` in the background, and prints the socket path only after the socket is already listening. The daemon accepts one LSP client at a time, keeps the upstream server warm while idle, and shuts it down after `--idle-timeout` (default `60s`). If the next client initializes with different normalized settings, lsp-cli restarts the upstream LSP server before serving that client.
`stop` resolves the same workspace/server selection as `daemon`, connects to the matching daemon socket, and asks that daemon to stop. If no matching daemon is active, the command succeeds and reports that nothing was running. When a workspace has multiple runnable detected languages and you do not pass `--lang` or `--lsp`, `stop` iterates over every matching language-specific daemon instead of failing on ambiguity.
`stop-all` scans `$XDG_RUNTIME_DIR/lsp-cli` and stops every active daemon it can reach, removing stale socket files along the way.
`update` downloads the `lsp-cli-data` GitHub release selected by `download-version` and installs it into `~/.local/share/lsp-cli/data/`. The archive is extracted into a temporary directory first, then all downloaded configs are validated before the installed data tree is replaced.
`languages` lists the configured language ids from the loaded `filetypes/` config. `servers` lists configured LSP server names from the loaded `lsp/` config, and `servers --lang LANG` narrows that list to one configured language.
LSP-backed commands such as `grep`, `list-symbols`, `references`, and `build-index` automatically prefer the matching daemon socket when it exists. If the socket file exists but no daemon is listening, lsp-cli removes the dead socket and falls back to starting the LSP server directly.
When no config data is available, lsp-cli automatically runs the same update flow once before the requested command. If that first automatic download or validation fails, the command fails too.


# References

- [LSP specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)


# Thanks

I'd like to say "thank you" to the following opensource projects:
- [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) was used to fill LSP servers database
- [mason](https://github.com/mason-org/mason.nvim) inspired me to implement LSP server autodownload
- [mason-registry](https://github.com/mason-org/mason-registry/) is used as an LSP server registry
