# v0.2

- repl (TODO: name... cli, console, terminal, interactive?)
- color?? pretty
- LSP server configs for all popular agents

- review lspconfig database
- review mason database
- user-visible messages - more friendly/informative -> into explicit module

- man: https://www.w3tutorials.net/blog/what-is-the-idiomatic-way-of-writing-man-pages-for-rust-cli-tools/
- explicit table of supported commands per LSP server (generate)

- design installer backends for the currently excluded RubyGems, LuaRocks, Open VSX, OPAM,
  Composer, and source-build Mason packages; the complete server inventory and required trust,
  cache, runtime, and test considerations are recorded in `docs/SERVERS.md`

- fill filetypes detection

# E2E

- code duplication
- disabled features
- make sure --download cache is used
- make download-e2e-deps && source activate-local-deps.inc
- fix github "no space" issue

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

--help subcommand grouping


# Bugs

- declaration for clangd drops std (e.g. `declaration f` drops `fgetc`)

# Implementation

- move all LSP command names (e.g. `workspace/symbol`) to a separate file
