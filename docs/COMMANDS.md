# Commands and options

[Back to README](../README.md)

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
  implementation       Find implementations of a symbol name
  type-definition      Find type definitions of a symbol name
  build-index          Wait for confirmed or configured best-effort workspace indexing
  update               Force update langages/servers database
  completion           Generate a shell completion script, write it to stdout
  agent-skill          Generate a generic Markdown skill file for code agents
  run                  Replace lsp-cli with the selected language server process
  help                 Print this message or the help of the given subcommand(s)

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
      --json         Print results as JSON.
      --no-json      Print human-readable output instead of JSON.
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
$ lsp-cli implementation --help
Find implementations of a symbol name

Usage: lsp-cli implementation [OPTIONS] <NAME> <DIRECTORY>

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
$ lsp-cli type-definition --help
Find type definitions of a symbol name

Usage: lsp-cli type-definition [OPTIONS] <NAME> <DIRECTORY>

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
Wait for confirmed or configured best-effort workspace indexing

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
$ lsp-cli agent-skill --help
Generate a generic Markdown skill file for code agents

Usage: lsp-cli agent-skill

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
