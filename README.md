# LSP-cli

LSP-cli is a simple tool allowing one to work with LSP servers from the terminal.
It can:
- detect filetypes in your worktree and suggest LSP server(s) cmdline
- use LSP server to make LSP requests (e.g. symbols, references, callers)
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

I'd like to say "thank you" to developers and maintainers
of [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) project,
which was used to fill LSP servers database.
