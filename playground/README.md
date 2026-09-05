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

To measure daemon forwarding overhead with a deterministic immediate-reply server:

```sh
cargo build
python3 scripts/daemon_latency.py --binary target/debug/lsp-cli --samples 100 --pipeline 16
```

Run this from the repository root. Add another `--binary /path/to/baseline/lsp-cli`
to compare builds; use `--skip-handshake-checks` for binaries predating independent
handshakes. The script uses `playground/rust` with temporary server
configuration and sockets; it requires Python 3 but no installed language server.
It reports direct and warm-daemon p50/p95/p99 request latency in milliseconds,
with sample counts for sequential requests and batches of 16 pipelined requests.
Initialization is outside the measured interval. It also checks busy rejection,
warm reuse, restart after capability changes or dynamic registration,
initialization-failure recovery, silent/trickling handshake expiration while active
requests continue, and stop behind a silent connection.

The daemon uses one outstanding event per reader or accept worker, keeping event
backlog bounded without a forwarding-loop polling delay. Individual message sizes
are not capped. First-message reads run independently with a two-second absolute
deadline and at most 16 pending handshakes. Excess connections are closed, including
stop connections when all slots are occupied. Writes, logging, and upstream
shutdown use per-output writer workers, so a peer that stops reading does not block
the coordinator. An output is flagged at 64 queued messages or 8 MiB of framed data;
it is unflagged after both values fall below their limits, or disconnected after the
configured `daemon.write-stall-timeout` (two seconds by default). Messages continue
to queue during that grace period, so memory use can temporarily exceed both limits.
Process start, shutdown, exit monitoring, and reaping run through a lifecycle worker;
logging remains synchronous. These measurements describe an immediate-reply fixture, not a
latency guarantee for real language servers or stalled peers.

The Lua playground exercises discovery of the local `normalize_timestamp` function,
which may be absent from LuaLS workspace-symbol results:

```sh
cargo run -- references normalize_timestamp playground/lua --lsp lua-language-server --detach
```

Compare runs with `max-requests-in-flight: 1` and `max-requests-in-flight: 20` in
`lsp-cli.yaml`. Both should report the call in `timestamp.lua`; the setting changes
how many file-symbol requests can be outstanding, not which files are searched.
