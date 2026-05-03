# Features

commands:
- repl (TODO: name... cli, console, terminal, interactive?)
- type hierarchy??
- semantic tokens??
- color?? pretty

- NO: hover?? - too noisy, doesn't make much sense
- NO: moniker?? - not usable outside of LSIF
- NO: code-lens?? - No, not very usable (trivial 'references', run go tidy, et.c)
- NO: inlay-hint?? - No, not useful
- NO: executeCommand?? - No, require edits from the client, mainly for refactoring

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
- threads for stderr buffering???


# Readme

- getting started
- configs location
- lsp-cli.yaml format
- examples
- how to use with AI code agents

man:
https://www.w3tutorials.net/blog/what-is-the-idiomatic-way-of-writing-man-pages-for-rust-cli-tools/


# Infrastructure

- search for useful BCPs among rust projects at github
- upload to crates.io


# Raw thoughts

- cover more initialize capabilities?
- cover more useful commands?
- ask GPT itself "how to word the skill file?"
- maybe use async-lsp?
