This directory contains small multi-file projects for manual `lsp-cli` experiments.

Each language playground is intentionally small but has enough structure to exercise:

- `detect`
- `list-files`
- `list-symbols`
- `list-functions`
- `grep`
- `definition`
- `declaration`
- `references`
- `callers`
- `callees`

Suggested commands:

```sh
cargo run -- detect playground/python
cargo run -- detect playground/python --lang python
cargo run -- detect playground/python --lsp pyright-langserver
cargo run -- detect playground/c --lang c
cargo run -- detect playground/cpp --lang cpp
cargo run -- grep Order playground/rust
cargo run -- list-symbols playground/c
cargo run -- list-symbols playground/java/src/main/java/playground/order/Order.java
cargo run -- definition format_order playground/c
cargo run -- references OrderFormatter playground/csharp
cargo run -- server-capabilities playground/rust --lsp rust-analyzer
cargo run -- daemon playground/python
cargo run -- stop playground/python
```

Check automatic server selection with
`cargo run -- definition format_order playground/c --lang c` or
`cargo run -- definition format_order playground/cpp --lang cpp`.
Server selection follows the configured preferences and server availability.

The projects reuse a similar domain across languages so symbol names are easy to remember
while trying different LSP servers.
