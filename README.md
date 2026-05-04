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
```

List symbols in one file or all matching files in a workspace:

```sh
lsp-cli list-symbols path/to/project/src/main.rs
lsp-cli list-symbols path/to/project
```

Collect diagnostics for the workspace:

```sh
lsp-cli diagnostics path/to/project
```

Format a file in place:

```sh
lsp-cli format path/to/file.rs
```

Use JSON output for scripts and code agents:

```sh
lsp-cli grep --json MySymbol path/to/project
```

## Commands and options

<!-- BEGIN GENERATED COMMANDS -->
```text
$ lsp-cli --help
Query language servers from the command line

Usage: lsp-cli <COMMAND>

Commands:
  commands             List canonical top-level subcommands
  daemon               Start a background daemon for the selected workspace and server
  stop                 Stop the matching background daemon (same cmdline and cwd)
  stop-all             Stop every active lsp-cli daemon (any cmdline, any cwd)
  languages            List known languages
  servers              List known LSP servers
  server-capabilities  Show the selected server's advertised capabilities
  detect               Detect runnable language servers for a path
  diagnostics          Print workspace diagnostics
  format               Format a file
  grep                 Search workspace symbols (regex syntax is server-dependent)
  list-symbols         List symbols from a file or workspace
  list-functions       List functions, methods, constructors, and operators
  list-files           List files that match the selected workspace filters
  references           Find references to a symbol name
  callers              Find callers of a symbol name
  callees              Find callees of a symbol name
  definition           Find definitions of a symbol name
  declaration          Find declarations of a symbol name
  build-index          Wait for the server to finish indexing a workspace
  update               Force update langages/servers database
  completion           Generate a shell completion script, write it to stdout
  run                  Replace lsp-cli with the selected language server process
  help                 Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

```text
$ lsp-cli commands --help
List canonical top-level subcommands

Usage: lsp-cli commands

Options:
  -h, --help  Print help
```

```text
$ lsp-cli daemon --help
Start a background daemon for the selected workspace and server

Usage: lsp-cli daemon [OPTIONS] [PATH]

Arguments:
  [PATH]  Path used to detect the workspace and server to daemonize. [default: .]

Options:
      --lang <LANG>       Select this language.
      --lsp <LSP>         Use a specific configured LSP server.
      --download          Download LSP server if not found in PATH.
      --no-download       Do not install missing servers automatically.
      --debug             Print verbose debug logs to stderr.
      --no-debug          Disable verbose debug logs.
      --idle-timeout <T>  Shut the daemon down after this much idle time.
  -h, --help              Print help
```

```text
$ lsp-cli stop --help
Stop the matching background daemon (same cmdline and cwd)

Usage: lsp-cli stop [OPTIONS] [PATH]

Arguments:
  [PATH]  Path used to resolve the daemon to stop. [default: .]

Options:
      --lang <LANG>  Select this language.
      --lsp <LSP>    Use a specific configured LSP server.
      --debug        Print verbose debug logs to stderr.
      --no-debug     Disable verbose debug logs.
  -h, --help         Print help
```

```text
$ lsp-cli stop-all --help
Stop every active lsp-cli daemon (any cmdline, any cwd)

Usage: lsp-cli stop-all [OPTIONS]

Options:
      --debug     Print verbose debug logs to stderr.
      --no-debug  Disable verbose debug logs.
  -h, --help      Print help
```

```text
$ lsp-cli languages --help
List known languages

Usage: lsp-cli languages

Options:
  -h, --help  Print help
```

```text
$ lsp-cli servers --help
List known LSP servers

Usage: lsp-cli servers [OPTIONS]

Options:
      --lang <LANG>  List servers configured for this language only.
  -h, --help         Print help
```

```text
$ lsp-cli server-capabilities --help
Show the selected server's advertised capabilities

Usage: lsp-cli server-capabilities [OPTIONS] <DIRECTORY>

Arguments:
  <DIRECTORY>  Workspace directory used to initialize the server.

Options:
      --lang <LANG>  Select this language.
      --lsp <LSP>    Use a specific configured LSP server.
      --detach       Use a background daemon socket when available, starting one if needed.
      --no-detach    Talk to the server in this process instead of using a background daemon.
      --download     Download LSP server if not found in PATH.
      --no-download  Do not install missing servers automatically.
      --debug        Print verbose debug logs to stderr.
      --no-debug     Disable verbose debug logs.
      --timeout <T>  Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
  -h, --help         Print help
```

```text
$ lsp-cli detect --help
Detect runnable language servers for a path

Usage: lsp-cli detect [OPTIONS] [PATH]

Arguments:
  [PATH]  Path to inspect for supported languages and servers. [default: .]

Options:
      --lang <LANG>  Select this language.
      --lsp <LSP>    Use a specific configured LSP server.
      --download     Download LSP server if not found in PATH.
      --no-download  Do not install missing servers automatically.
      --json         Print results as JSON.
      --no-json      Print human-readable output instead of JSON.
  -q                 Print only the suggested server command lines.
      --no-quiet     Print labeled output instead of only command lines.
      --debug        Print verbose debug logs to stderr.
      --no-debug     Disable verbose debug logs.
  -h, --help         Print help
```

```text
$ lsp-cli diagnostics --help
Print workspace diagnostics

Usage: lsp-cli diagnostics [OPTIONS] <DIRECTORY>

Arguments:
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
  -h, --help                Print help
```

```text
$ lsp-cli format --help
Format a file

Usage: lsp-cli format [OPTIONS] <PATH>

Arguments:
  <PATH>  File to format.

Options:
      --lang <LANG>  Select this language.
      --lsp <LSP>    Use a specific configured LSP server.
      --download     Download LSP server if not found in PATH.
      --no-download  Do not install missing servers automatically.
      --detach       Use a background daemon socket when available, starting one if needed.
      --no-detach    Talk to the server in this process instead of using a background daemon.
      --json         Print results as JSON.
      --no-json      Print human-readable output instead of JSON.
      --debug        Print verbose debug logs to stderr.
      --no-debug     Disable verbose debug logs.
      --timeout <T>  Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --check        Exit with an error if formatting would change the file.
      --stdout       Write the formatted file to stdout instead of modifying it.
  -h, --help         Print help
```

```text
$ lsp-cli grep --help
Search workspace symbols (regex syntax is server-dependent)

Usage: lsp-cli grep [OPTIONS] <PATTERN> <DIRECTORY>

Arguments:
  <PATTERN>    Pattern to send to `workspace/symbol`.
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
  -h, --help                Print help
```

```text
$ lsp-cli list-symbols --help
List symbols from a file or workspace

Usage: lsp-cli list-symbols [OPTIONS] <PATH>

Arguments:
  <PATH>  File or directory whose symbols to list.

Options:
      --lang <LANG>     Select this language.
      --lsp <LSP>       Use a specific configured LSP server.
      --detach          Use a background daemon socket when available, starting one if needed.
      --no-detach       Talk to the server in this process instead of using a background daemon.
      --wait-for-index  Wait for background indexing before sending the workspace query.
      --download        Download LSP server if not found in PATH.
      --no-download     Do not install missing servers automatically.
      --json            Print results as JSON.
      --no-json         Print human-readable output instead of JSON.
      --debug           Print verbose debug logs to stderr.
      --no-debug        Disable verbose debug logs.
      --timeout <T>     Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>       Maximum number of results to print. Mainly usable for code agents.
  -h, --help            Print help
```

```text
$ lsp-cli list-functions --help
List functions, methods, constructors, and operators

Usage: lsp-cli list-functions [OPTIONS] <DIRECTORY>

Arguments:
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
  -h, --help                Print help
```

```text
$ lsp-cli list-files --help
List files that match the selected workspace filters

Usage: lsp-cli list-files [OPTIONS] <DIRECTORY>

Arguments:
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>     Select this language.
      --lsp <LSP>       Use a specific configured LSP server.
      --wait-for-index  Wait for background indexing before sending the workspace query.
      --json            Print results as JSON.
      --no-json         Print human-readable output instead of JSON.
      --debug           Print verbose debug logs to stderr.
      --no-debug        Disable verbose debug logs.
      --timeout <T>     Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>       Maximum number of results to print. Mainly usable for code agents.
  -h, --help            Print help
```

```text
$ lsp-cli references --help
Find references to a symbol name

Usage: lsp-cli references [OPTIONS] <NAME> <DIRECTORY>

Arguments:
  <NAME>       Symbol name to search for.
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
  -h, --help                Print help
```

```text
$ lsp-cli callers --help
Find callers of a symbol name

Usage: lsp-cli callers [OPTIONS] <NAME> <DIRECTORY>

Arguments:
  <NAME>       Symbol name to search for.
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
  -h, --help                Print help
```

```text
$ lsp-cli callees --help
Find callees of a symbol name

Usage: lsp-cli callees [OPTIONS] <NAME> <DIRECTORY>

Arguments:
  <NAME>       Symbol name to search for.
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
  -h, --help                Print help
```

```text
$ lsp-cli definition --help
Find definitions of a symbol name

Usage: lsp-cli definition [OPTIONS] <NAME> <DIRECTORY>

Arguments:
  <NAME>       Symbol name to search for.
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
      --full                Include full source text for each match in output.
  -h, --help                Print help
```

```text
$ lsp-cli declaration --help
Find declarations of a symbol name

Usage: lsp-cli declaration [OPTIONS] <NAME> <DIRECTORY>

Arguments:
  <NAME>       Symbol name to search for.
  <DIRECTORY>  Workspace directory to query.

Options:
      --lang <LANG>         Select this language.
      --lsp <LSP>           Use a specific configured LSP server.
      --wait-for-index      Wait for background indexing before sending the workspace query.
      --json                Print results as JSON.
      --no-json             Print human-readable output instead of JSON.
      --debug               Print verbose debug logs to stderr.
      --no-debug            Disable verbose debug logs.
      --timeout <T>         Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
      --limit <N>           Maximum number of results to print. Mainly usable for code agents.
      --download            Download LSP server if not found in PATH.
      --no-download         Do not install missing servers automatically.
      --detach              Use a background daemon socket when available, starting one if needed.
      --no-detach           Talk to the server in this process instead of using a background daemon.
  -l, --files-with-matches  Print only file paths that contain matches.
      --full                Include full source text for each match in output.
  -h, --help                Print help
```

```text
$ lsp-cli build-index --help
Wait for the server to finish indexing a workspace

Usage: lsp-cli build-index [OPTIONS] <DIRECTORY>

Arguments:
  <DIRECTORY>  Workspace directory to index.

Options:
      --lang <LANG>  Select this language.
      --lsp <LSP>    Use a specific configured LSP server.
      --detach       Use a background daemon socket when available, starting one if needed.
      --no-detach    Talk to the server in this process instead of using a background daemon.
      --download     Download LSP server if not found in PATH.
      --no-download  Do not install missing servers automatically.
      --debug        Print verbose debug logs to stderr.
      --no-debug     Disable verbose debug logs.
      --timeout <T>  Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.
  -h, --help         Print help
```

```text
$ lsp-cli update --help
Force update langages/servers database

Usage: lsp-cli update

Options:
  -h, --help  Print help
```

```text
$ lsp-cli completion --help
Generate a shell completion script, write it to stdout

Usage: lsp-cli completion [SHELL]

Arguments:
  [SHELL]  Shell to generate completion for. Defaults to the current shell from $SHELL. [possible values: bash, elvish, fish, powershell, zsh]

Options:
  -h, --help  Print help
```

```text
$ lsp-cli run --help
Replace lsp-cli with the selected language server process

Usage: lsp-cli run [OPTIONS] [PATH]

Arguments:
  [PATH]  Path used to detect the workspace and server to run. [default: .]

Options:
      --lang <LANG>  Select this language.
      --lsp <LSP>    Use a specific configured LSP server.
      --download     Download LSP server if not found in PATH.
      --no-download  Do not install missing servers automatically.
      --debug        Print verbose debug logs to stderr.
      --no-debug     Disable verbose debug logs.
  -h, --help         Print help
```
<!-- END GENERATED COMMANDS -->

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
