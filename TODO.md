# Features

commands:
- repl (TODO: name... cli, console, terminal, interactive?)
- diagnostics
- format
- moniker??
- type hierarchy??
- semantic tokens??
- color?? pretty
- code-lens??
- hover??
- inlay-hint??
- executeCommand??

options:
- -s|--signature - show the full signature

generic:
- lsp commands should spawn ALL discovered LSP servers in case of multiple languages


# Bugs

- review lspconfig database
- review mason database
- global log

- declaration for clangd drops std (e.g. `declaration f` drops `fgetc`)


# Source code

- user-visible messages - more friendly/informative -> into explicit module
- improve readme (integration with agents/IDE)
- threads for stderr buffering???


# Infrastructure

- search for useful BCPs among rust projects at github
- upload to crates.io


# Raw thoughts

- cover more initialize capabilities?
- cover more useful commands?
- ask GPT itself "how to word the skill file?"
- maybe use async-lsp?
- link to LSP spec in README.md?

