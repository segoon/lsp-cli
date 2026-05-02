# Features

commands:
- repl (TODO: name... cli, console, terminal, interactive?)

options:
- -s|--signature - show the full signature
- -b|--body - show the full function body

generic:
- lsp commands should spawn ALL discovered LSP servers in case of multiple languages


# Bugs

- show stderr on initialize failure
- review lspconfig database
- review mason database


# Source code

- duplication (vec![x.to_string(), ...])
- user-visible messages - more friendly/informative -> into explicit module
- improve readme (integration with agents/IDE)



# Raw thoughts

- cover more initialize capabilities?
- cover more useful commands?
- ask GPT itself "how to word the skill file?"
- maybe use async-lsp?
- link to LSP spec in README.md?
