# v0.2

- repl (TODO: name... cli, console, terminal, interactive?)
- color?? pretty

- review lspconfig database
- review mason database
- user-visible messages - more friendly/informative -> into explicit module

- readme
- man
- informative --help
- better completion (--lsp, --lang)
- agent-skill

- test on multiple rust versions


# Features

commands:
- type hierarchy??
- semantic tokens??

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

- declaration for clangd drops std (e.g. `declaration f` drops `fgetc`)


# Readme

- getting started
- configs location
- lsp-cli.yaml format
- examples
- how to use with AI code agents

man:
https://www.w3tutorials.net/blog/what-is-the-idiomatic-way-of-writing-man-pages-for-rust-cli-tools/


# Infrastructure

- upload to crates.io
