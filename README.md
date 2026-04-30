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


# Available modes

```sh
# Suggest LSP server cmdline based on filenames
lsp-cli detect path/to/project
lsp-cli detect path/to/project/main.py --json

# Query workspace symbols through the first detected LSP server
lsp-cli grep MySymbol path/to/project
lsp-cli grep --json MySymbol path/to/project
```

`grep` uses the LSP `workspace/symbol` request. Pattern syntax and matching behavior are server-dependent.


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
