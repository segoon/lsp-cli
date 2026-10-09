# lsp-cli

[![CI](https://github.com/segoon/lsp-cli/actions/workflows/ci.yml/badge.svg?branch=master&event=push)](https://github.com/segoon/lsp-cli/actions/workflows/ci.yml?query=branch%3Amaster+event%3Apush)
[![Nightly compatibility](https://github.com/segoon/lsp-cli/actions/workflows/e2e.yml/badge.svg?branch=master&event=schedule)](https://github.com/segoon/lsp-cli/actions/workflows/e2e.yml?query=branch%3Amaster+event%3Aschedule)
[![Dependency audit](https://github.com/segoon/lsp-cli/actions/workflows/audit.yml/badge.svg?branch=master&event=schedule)](https://github.com/segoon/lsp-cli/actions/workflows/audit.yml?query=branch%3Amaster+event%3Aschedule)

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

### Install via cargo

The preferred installation method.

Assuming you already have cargo installed, install `lsp-cli` with Cargo:

```sh
cargo install lsp-cli
```

Check that it works:
```
lsp-cli --help
```

Generate `SKILL.md` for your code agent (if any):
```
mkdir -p .claude/skills/lsp-cli/
lsp-cli agent-skill >.claude/skills/lsp-cli/SKILL.md
```

### Compiling from sources

Clone the repository with its data files and run:

```sh
git clone --recurse-submodules https://github.com/segoon/lsp-cli.git
cd lsp-cli
cargo build

# Run the debug executable from ./target/debug/
cargo run -- detect playground/python
cargo run -- grep Order playground/rust
cargo run -- definition format_order playground/c --lsp clangd
```

### Test and compatibility policy

Pull requests run `make check` plus a required, explicitly tagged smoke suite. Nightly and
manually dispatched **End-to-end
compatibility** workflows run every supported pair. A red required job therefore indicates a
regression or newly incompatible upstream release; known unsupported pairs are explicit reviewed
exclusions rather than silently skipped tests.

The split keeps pull-request feedback reasonably fast, but a regression in a non-preferred server
may first appear in the nightly run. See [the E2E guide](tests/e2e/Readme.md) for local commands,
manual workflow selectors, prerequisites, caching, and exclusion details.


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
5. (downloads an LSP server if it is missing)[DOWNLOAD.md]
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

Find definitions, declarations, implementations, type definitions, and references by symbol name:

```sh
lsp-cli definition MySymbol path/to/project
lsp-cli declaration MySymbol path/to/project
lsp-cli implementation MySymbol path/to/project
lsp-cli type-definition MySymbol path/to/project
lsp-cli references MySymbol path/to/project
```

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

A new connection has two seconds to send its first complete message. Silent or
incomplete connections are closed without interrupting the active client. Up to
16 connections can wait to start a session; additional connections are closed,
including `stop` connections while all 16 slots are occupied.


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

# Maximum outstanding document-symbol requests while finding a symbol by name
# (references, definition, declaration, implementation, type-definition, callers,
# and callees). Default: 20.
# Use 1 for sequential requests or a smaller value to reduce server load.
max-requests-in-flight: 20
# A file-symbol timeout fails the query instead of returning incomplete results.

# Default maximum number of printed results.
# Useful for code agents.
limit: 100

detect:
  # Print only suggested command lines for `detect`.
  quiet: false

daemon:
  # Shut down an idle daemon after this much time.
  idle-timeout: "60"
  # Disconnect an output peer that remains backlogged for this long. Default: 2 seconds.
  write-stall-timeout: "2"

lsp:
  # Example server preference list for C++.
  cpp:
    - clangd

  # Prefer these Python servers in this order.
  python:
    - ty
    - pyright-langserver
    - jedi-language-server
```

Each daemon output uses an ordered writer queue. A queue is marked stalled at 64
messages or 8 MiB of framed data and unmarked after both values fall below those
limits. The daemon continues accepting messages during the grace period, so memory
can grow if traffic continues; it disconnects a slow client or stops an unresponsive
server if the queue stays marked for `write-stall-timeout`. A message larger than
8 MiB is allowed and begins writing after earlier messages on that output.
Server restart and shutdown also advance through daemon events. The daemon keeps
accepting control traffic while a process starts or exits; an unresponsive server
is force-stopped after the existing two-second shutdown and exit deadlines.
Daemon traffic, lifecycle, error, and captured-server-stderr logs use a 64-record
worker queue, so formatting, stderr, and the global log file do not block forwarding.
New records are dropped when this queue is full, and the daemon reports the count
after logging resumes. Normal shutdown waits at most 100 milliseconds for logs.

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

# Use only for servers known not to expose a terminal indexing signal. The default is confirmed.
build-index-completion: best-effort
```

`build-index-completion: best-effort` keeps the command's bounded wait and still reports transport,
protocol, server, and shutdown errors. It only treats expiry without a terminal progress signal as
a successful best-effort attempt; success does not confirm that the whole workspace was indexed.
The maintained best-effort server list is in `docs/SERVERS.md`.

An LSP config can declare `mason-extra-packages` when its Mason package has an incomplete
dependency constraint. With automatic downloads, lsp-cli installs these package specifications in
the same npm or Python environment as the server. For example, CMake Language Server currently
uses `pygls<2` because its released code imports the pygls 1.x API. This is a compatibility escape
hatch; prefer an upstream package constraint when one is available.

## Commands and options

See the [command reference](docs/COMMANDS.md) for all commands and options.

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
lsp-cli implementation format_order playground/c --lsp clangd
lsp-cli type-definition format_order playground/c --lsp clangd
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

Generate a generic Markdown skill file for a code agent:

```sh
lsp-cli agent-skill > SKILL.md
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

# Links

- lsp-cli on [github](https://github.com/segoon/lsp-cli) and on [crate.io](https://crates.io/crates/lsp-cli)
- lsp-cli-data on [github](https://github.com/segoon/lsp-cli)


# References

- [LSP specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)

# Thanks

I'd like to say "thank you" to the following opensource projects:
- [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) was used to fill LSP servers database
- [mason](https://github.com/mason-org/mason.nvim) inspired me to implement LSP server autodownload
- [mason-registry](https://github.com/mason-org/mason-registry/) is used as an LSP server registry
