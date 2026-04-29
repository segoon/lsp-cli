This is a work-in-progress. Expect some glitches.


# LSP-cli

LSP-cli is a simple tool allowing one to work with LSP servers from the terminal with minimal configuration.

It is able to:
- detect filetypes in your worktree
- select LSP server responsible for the filetype handling
- download LSP server
- suggest LSP server(s) cmdline
- start LSP server / pretend to be an LSP server (proxying requests to the real LSP server)
- as LSP cli: use LSP server to make LSP requests (e.g. symbols, references, callers)
- handle LSP server housekeeping (start/stop) for an external LSP client


# Quick start

```sh
git clone blablabla
cargo run
```


# Available modes

```sh
# Suggest LSP server cmdline based on filenames
lsp-cli detect path/to/project
lsp-cli detect path/to/project/main.py
lsp-cli detect --json main.cpp
```


# Thanks

I'd like to say "thank you" to the following opensource projects:
- [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) was used to fill LSP servers database
- [mason](https://github.com/mason-org/mason.nvim) inspired me to implement LSP server autodownload


# TODO

core:
- download LSP server
- start LSP server / pretend to be an LSP server (proxying requests to the real LSP server)
- as LSP cli: use LSP server to make LSP requests (e.g. symbols, references, callers)
- handle LSP server housekeeping (start/stop) for an external LSP client

detect:
- multiple LSP servers for the same filetype: priority
- output file count
- LSP servers priority among server for the same filetype

commands:
- symbol|search|find
- symbol-definition
- symbol-declaration
- references|refs
- callers
- callees
- symbols-file
- symbols-workspace
- repl (TODO: name... cli, console, terminal, interactive?)

options:
- -s|--signature - show the full signature
- -b|--body - show the full function body
- -l|--limit - output limit in lines

lifecycle:
- start+stop - in background
- status
- addr - show active LSP server
- serve - synch start LSP server, stop it on exit (TODO: what to do with multiple LSP servers?)
