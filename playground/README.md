# Playground projects

This directory contains small projects for reproducing `lsp-cli` behavior manually. Run commands
from the repository root. Build the current binary and list the configured servers for a language
before choosing one:

```sh
cargo build
cargo run -- servers --lang cuda
```

The examples below use one server name for orientation, not as the only supported choice. Replace
it with any compatible server printed by `servers --lang`. The selected executable must already be
available in `PATH`; remove `--no-download` only when its configuration supports downloading.

## Source-language workflow

The newest source projects share stable order-domain symbols:

| Language | Project | Example server | File | Type | Function |
|---|---|---|---|---|---|
| CUDA | `playground/cuda` | `clangd` | `src/main.cu` | `Order` | `format_order` |
| Kotlin | `playground/kotlin` | `kotlin-language-server` | `src/main/kotlin/playground/App.kt` | `Order` | `formatOrder` |
| Objective-C | `playground/objc` | `clangd` | `src/main.m` | `Order` | `format_order` |
| Objective-C++ | `playground/objcpp` | `clangd` | `src/main.mm` | `Order` | `format_order` |

Set the values from one row. This CUDA example can be changed to any other row without changing
the commands below:

```sh
language=cuda
project=playground/cuda
server=clangd
file="$project/src/main.cu"
symbol=Order
function=format_order
```

Start with detection and file selection. These commands do not start a daemon:

```sh
cargo run -- detect "$project" --lang "$language" --lsp "$server" --no-download
cargo run -- list-files "$project" --lang "$language" --lsp "$server"
```

Use a small shell helper to run each request directly against the chosen server:

```sh
lsp_cli() {
  cargo run -- "$@" --lang "$language" --lsp "$server" --no-download --no-detach
}

lsp_cli server-capabilities "$project"
lsp_cli diagnostics "$project"
lsp_cli build-index "$project"
lsp_cli list-symbols "$project"
lsp_cli list-functions "$project"
lsp_cli grep "$symbol" "$project"
lsp_cli definition "$function" "$project"
lsp_cli declaration "$function" "$project"
lsp_cli references "$function" "$project"
lsp_cli callers "$function" "$project"
lsp_cli callees "$function" "$project"
lsp_cli format "$file" --stdout
```

`format --stdout` leaves the tracked project unchanged. Formatting, declarations, diagnostics,
workspace symbols, and call hierarchy are optional LSP capabilities. A clear unsupported-capability
error is therefore a useful result when the selected server does not advertise an operation.

Exercise daemon reuse separately:

```sh
cargo run -- daemon "$project" --lang "$language" --lsp "$server" --no-download
cargo run -- server-capabilities "$project" --lang "$language" --lsp "$server" --no-download --detach
cargo run -- stop "$project" --lang "$language" --lsp "$server"
```

## Go metadata workflow

The metadata-only fixtures deliberately isolate filename detection:

| Language | Project | Example server |
|---|---|---|
| Go module metadata | `playground/gomod` | `gopls` |
| Go workspace metadata | `playground/gowork` | `gopls` |

Set `language` to `gomod` or `gowork`, then use the corresponding project:

```sh
language=gomod
project=playground/gomod
server=gopls

cargo run -- servers --lang "$language"
cargo run -- detect "$project" --lang "$language" --lsp "$server" --no-download
cargo run -- list-files "$project" --lang "$language" --lsp "$server"
cargo run -- server-capabilities "$project" --lang "$language" --lsp "$server" --no-download --no-detach
cargo run -- daemon "$project" --lang "$language" --lsp "$server" --no-download
cargo run -- stop "$project" --lang "$language" --lsp "$server"
```

These fixtures can exercise detection, file listing, server initialization, and lifecycle. They do
not contain source symbols, so symbol, reference, formatting, diagnostic, and call-hierarchy
commands have no meaningful metadata-only expectation.

## Existing project examples

The older playgrounds use the same order domain where it is natural. A few useful starting points
are:

```sh
cargo run -- grep Order playground/rust --lsp rust-analyzer --no-detach
cargo run -- list-symbols playground/java/src/main/java/playground/order/Order.java
cargo run -- definition format_order playground/c --lang c
cargo run -- references OrderFormatter playground/csharp
```

The Lua playground instead exercises discovery of the local `normalize_timestamp` function, which
may be absent from LuaLS workspace-symbol results:

```sh
cargo run -- references normalize_timestamp playground/lua --lsp lua-language-server --detach
```

Compare runs with `max-requests-in-flight: 1` and `max-requests-in-flight: 20` in `lsp-cli.yaml`.
Both should report the call in `timestamp.lua`; the setting changes how many file-symbol requests
can be outstanding, not which files are searched.

## Daemon latency workflow

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
daemon logging uses a bounded worker and a 100-millisecond shutdown flush budget.
The check holds the global log lock while forwarding and while stopping, and verifies
that overflow is reported after the lock is released. These measurements describe
an immediate-reply fixture, not a latency guarantee for real language servers or
stalled peers.
