# lsp-cli

`lsp-cli` is a command-line tool for talking to Language Server Protocol (LSP) servers from the terminal without an editor.

It helps you do editor-style code navigation and inspection from a terminal:

- detect which language and LSP server fit a project
- download LSP server if it is missing in the system
- call simple LSP commands like references, callers, list-symbols
- collect diagnostics
- format files
- inspect server capabilities
- keep a server warm in a background daemon

The goal is simple: point `lsp-cli` at a file or project directory, let it choose a matching LSP server, and query that server from the shell.


## Getting Started

Clone the repository with its data files:

```sh
git clone --recurse-submodules https://github.com/segoon/lsp-cli.git
cd lsp-cli
```

Build and try a few commands:

```sh
cargo run -- detect playground/python
cargo run -- grep Order playground/rust
cargo run -- definition format_order playground/c --lsp clangd
```

If you cloned the repository earlier without submodules, initialize them first:

```sh
git submodule update --init --recursive
```

If config data is not available yet, most commands try to install it automatically before running.


## Use cases

You might want to use `lsp-cli` if:

- you want to make simple LSP requests from the terminal
- your code agent doesn't support a rare/proprietary LSP server (e.g. for a rare language)
- you implement an IDE/editor that should support a wide range of languages, but you don't want to mess around LSP server configuration / distribution


## What lsp-cli Does

At a high level, `lsp-cli` usually does the following:

1. scans the given directory
2. detects matching languages from filenames
3. chooses one or more configured LSP servers for those languages
4. chooses a workspace root for the selected server
5. downloads an LSP server if it is missing
6. starts the server, or reuses a matching daemon
7. sends the requested LSP query
8. prints a human-readable result or JSON

This means you do not have to assemble server command lines by hand or even install LSP server at all.

## Typical Workflows

Detect what `lsp-cli` would run:

```sh
lsp-cli detect path/to/project
lsp-cli detect --lang python path/to/project
```

Search for symbols across a workspace:

```sh
lsp-cli grep MySymbol path/to/project
```

Find definitions, declarations, and references by symbol name:

```sh
lsp-cli definition MySymbol path/to/project
lsp-cli declaration MySymbol path/to/project
lsp-cli references MySymbol path/to/project

Find callers and callees by function name:

```sh
lsp-cli callers format_order path/to/project
lsp-cli callees format_order path/to/project
```

List symbols in one file or all matching files in a workspace:

```sh
lsp-cli list-symbols path/to/project/src/main.rs
lsp-cli list-symbols path/to/project
```

List functions:

```sh
lsp-cli list-functions path/to/project
```

Collect diagnostics for the workspace:

```sh
lsp-cli diagnostics path/to/project
```

Format a file:

```sh
lsp-cli format path/to/file.rs

# exits with status 0 if the files is already formatted
lsp-cli format --check path/to/file.rs

# do not change the original file, the formatted content is written to stdout
lsp-cli format --stdout path/to/file.rs
```

Use JSON output for scripts and code agents:

```sh
lsp-cli grep --json MySymbol path/to/project
```

Inspect the selected server capabilities:

```sh
lsp-cli server-capabilities --lsp rust-analyzer path/to/project
```

You may start an LSP server in background to reuse it later:

```sh
# start
lsp-cli daemon playground/python

# reuse the existing background daemon, do not spawn a new one
lsp-cli references playground/python

# stop a specific daemon
lsp-cli stop playground/python

# stop all background daemons
lsp-cli stop-all
```

The same background daemon is spawned and left idle after `lsp-cli <CMD> --detach` is finished.


## Configuration Files

If you want to customize `lsp-cli`, the three important config layers are:

- `lsp-cli.yaml` for defaults and preferences (especially LSP server preference order)
- `filetypes/*.yaml` for language detection
- `lsp/*.yaml` for server definitions

### Data Root

The data root is `~/.local/share/lsp-cli/data`.
It contains:

- `filetypes/*.yaml` - language detection settings
- `lsp/*.yaml` - LSP server settings
- `lsp-cli.yaml`

### User Configuration File

User-specific overrides live in a separate file:

`$XDG_CONFIG_HOME/lsp-cli/lsp-cli.yaml` (or `~/.config/lsp-cli/lsp-cli.yaml` if `$XDG_CONFIG_HOME` is empty).


### Precedence

Settings are applied in this order:

1. global `lsp-cli.yaml` from the data root
2. user `lsp-cli.yaml`
3. command-line flags

User config overrides global config. Command-line flags override both.

## lsp-cli.yaml

`lsp-cli.yaml` stores CLI defaults and server preference order:

```yaml
# Install a missing server automatically when a command needs it.
download: false

# Which lsp-cli data release `update` should install.
download-version: latest

# Reuse or start background daemons for LSP-backed commands.
detach: false

# Print JSON.
json: false

# Print verbose logs and raw LSP traffic to stderr.
debug: false

# Default per-request timeout.
# It supports two formats:
# - 10.1 means 10.1 seconds
# - 100ms means 0.1 seconds
timeout: "10"

# Default maximum number of printed results.
# Useful for code agents.
limit: 100

detect:
  # Print only suggested command lines for `detect`.
  quiet: false

daemon:
  # Shut down an idle daemon after this much time.
  idle-timeout: "60"

lsp:
  # Prefer clangd for C++.
  cpp:
    - clangd

  # Prefer these Python servers in this order.
  python:
    - ty
    - pyright-langserver
    - jedi-language-server
```

## Language Configs: filetypes/*.yaml

Files in `filetypes/` define how `lsp-cli` recognizes a language.

The filename becomes the language id:

- `filetypes/python.yaml` defines the `python` language
- `filetypes/cpp.yaml` defines the `cpp` language

Example:

```yaml
# filetypes/python.yaml

# Match files by extension.
extensions:
  - "py"

# Match files by filename regex when needed.
patterns: []
```

Another example for a filename-based language:

```yaml
# filetypes/BUILD.bazel.yaml

# No extension-based matching.
extensions: []

# Match special filenames.
patterns:
  - "^BUILD(\\.bazel)?$"
```

## LSP Server Configs: lsp/*.yaml

Files in `lsp/` define how `lsp-cli` can run a language server.

`cmdline` may include `$WORKSPACE`, which is replaced with the resolved workspace path

The workspace root is identified the following way.
First, `lsp-cli` walks upward until it finds one of the configured `root_markers`.
If no marker is found, it uses the input directory or input file parent directory as the workspace root.

Example:

```yaml
# lsp/pyright.yaml

# Languages this server handles.
filetypes:
  - "python"

# Search upward for these files to choose the workspace root.
root_markers:
  - "pyrightconfig.json"
  - "pyproject.toml"
  - "setup.py"
  - ".git"

# User-visible server name.
# This is the value used with `--lsp`.
name: "pyright-langserver"

# Command used to start the server.
cmdline: "pyright-langserver --stdio"
```

Example with `$WORKSPACE` and indexing behavior:

```yaml
# lsp/clangd.yaml

filetypes:
  - "c"
  - "cpp"

root_markers:
  - ".clangd"
  - "compile_commands.json"
  - ".git"

name: "clangd"

# `$WORKSPACE` is replaced with the resolved workspace path.
cmdline: "clangd --background-index --compile-commands-dir=$WORKSPACE"

# Whether commands that can wait for indexing should do so by default.
wait-for-index: false
```

## Commands and options

<!-- BEGIN GENERATED COMMANDS -->
```text
```
<!-- END GENERATED COMMANDS -->

## Useful Examples

Detect candidate servers for a project:

```sh
lsp-cli detect playground/python
lsp-cli detect playground/python --lang python
lsp-cli detect playground/python --lsp pyright-langserver
```

Search workspace symbols:

```sh
lsp-cli grep Order playground/rust
lsp-cli grep --json Order playground/rust
```

List symbols and functions:

```sh
lsp-cli list-symbols playground/java/src/main/java/playground/order/Order.java
lsp-cli list-functions playground/rust
```

Find locations and call relationships:

```sh
lsp-cli definition format_order playground/c --lsp clangd
lsp-cli declaration format_order playground/c --lsp clangd
lsp-cli references OrderFormatter playground/csharp
lsp-cli callers format_order playground/c --lsp clangd
lsp-cli callees format_order playground/c --lsp clangd
```

Diagnostics and formatting:

```sh
lsp-cli diagnostics playground/python
lsp-cli diagnostics --json playground/python
lsp-cli format playground/rust/src/main.rs
lsp-cli format --check playground/rust/src/main.rs
lsp-cli format --stdout playground/rust/src/main.rs
```

Inspect the selected server:

```sh
lsp-cli server-capabilities playground/rust --lsp rust-analyzer
lsp-cli build-index playground/rust --lsp rust-analyzer
```

Use background daemons:

```sh
lsp-cli daemon playground/python
lsp-cli stop playground/python
lsp-cli stop-all
```

List known languages and servers:

```sh
lsp-cli languages
lsp-cli servers
lsp-cli servers --lang python
```

Generate shell completion:

```sh
lsp-cli completion bash > /tmp/lsp-cli.bash
```

## Playground

The repository contains small multi-file sample projects in `playground/` for manual testing.

They are useful for learning what each command prints before pointing `lsp-cli` at a real project.

## Limitations

- `lsp-cli` works through LSP servers, so results are only as good as the selected server
- not every server supports every feature
- some features may silently fail with some LSP servers
- symbol search quality (and regex syntax) varies between servers
- background indexing support varies between servers

# References

- [LSP specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)

# Thanks

I'd like to say "thank you" to the following opensource projects:
- [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) was used to fill LSP servers database
- [mason](https://github.com/mason-org/mason.nvim) inspired me to implement LSP server autodownload
- [mason-registry](https://github.com/mason-org/mason-registry/) is used as an LSP server registry
