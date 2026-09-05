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

The Lua playground exercises discovery of the local `normalize_timestamp` function,
which may be absent from LuaLS workspace-symbol results:

```sh
cargo run -- references normalize_timestamp playground/lua --lsp lua-language-server --detach
```

Compare runs with `max-requests-in-flight: 1` and `max-requests-in-flight: 20` in
`lsp-cli.yaml`. Both should report the call in `timestamp.lua`; the setting changes
how many file-symbol requests can be outstanding, not which files are searched.
