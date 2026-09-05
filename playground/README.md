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
cargo run -- grep Order playground/rust
cargo run -- list-symbols playground/c
cargo run -- list-symbols playground/java/src/main/java/playground/order/Order.java
cargo run -- definition format_order playground/c
cargo run -- references OrderFormatter playground/csharp
cargo run -- server-capabilities playground/rust --lsp rust-analyzer
cargo run -- daemon playground/python
cargo run -- stop playground/python
```

The projects reuse a similar domain across languages so symbol names are easy to remember
while trying different LSP servers.

To measure daemon forwarding overhead with a deterministic immediate-reply server:

```sh
cargo build
python3 scripts/daemon_latency.py --binary target/debug/lsp-cli --samples 100 --pipeline 16
```

Run this from the repository root. Add another `--binary /path/to/baseline/lsp-cli`
to compare builds. The script uses `playground/rust` with temporary server
configuration and sockets; it requires Python 3 but no installed language server.
It reports direct and warm-daemon p50/p95/p99 request latency in milliseconds,
with sample counts for sequential requests and batches of 16 pipelined requests.
Initialization is outside the measured interval. It also checks busy rejection,
warm reuse, restart after capability changes or dynamic registration,
initialization-failure recovery, and stop with an active client.

The daemon uses one outstanding event per reader or accept worker, keeping event
backlog bounded without a forwarding-loop polling delay. Individual message sizes
are not capped, and handshakes, writes, logging, and upstream shutdown still run
synchronously. These measurements describe an immediate-reply fixture, not a
latency guarantee for real language servers or stalled peers.
