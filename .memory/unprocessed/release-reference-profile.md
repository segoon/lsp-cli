# Release references benchmark and remaining latency (2026-09-05)

## User correction

The user asked to investigate remaining latency and explicitly corrected the use of
**debug** benchmarks: use **release**. Future performance comparisons must build with
`cargo build --release` and invoke `target/release/lsp-cli`. No new product preference
or architectural decision was requested in this investigation.

## Measurements

Built the current uncommitted implementation in release. Ran without --debug against
/home/segoon/projects/parley.nvim, using isolated config/runtime directories and the
installed LuaLS executable. All six reference outputs were identical:

| Request window | Run 1 | Run 2 | Run 3 | Median |
|---|---:|---:|---:|---:|
| 1 | 19.207 s | 21.177 s | 20.266 s | 20.266 s |
| 20 | 12.982 s | 13.438 s | 13.003 s | 13.003 s |

These are repeated runs, not guaranteed warm-server runs. The project currently has
193 Lua files, compared with 188 in the earlier debug investigation, so the old and
new series are not a controlled comparison of compiler profiles alone.

## Remaining time

A temporary Python pass-through wrapper recorded message timestamps, payload sizes,
and Linux /proc CPU counters for LuaLS without verbose JSON logging. Two release
queries with a window of 20 took 12.809/13.211 seconds:

- 193 documentSymbol requests: span 12.425/12.774 seconds at the server boundary.
- LuaLS CPU consumed across that span: 12.16/12.61 seconds.
- Foreground CLI user+system CPU: 0.574/0.626 seconds (excludes daemon CPU).
- Each run returned 5.509 MB of document-symbol JSON and about 300 diagnostic notifications.
- Actual references request at the server boundary: 0.0034/0.0030 seconds.
- Maximum observed outstanding documentSymbol requests: 20.

Request latencies overlap and must not be summed as elapsed time. Server CPU and
foreground CPU also overlap; the figures are not additive phase timings. Wrapper
measurements have instrumentation overhead; the uninstrumented series above is the
baseline.

## Controlled diagnostics comparison

Used LuaLS --configpath pointing to a temporary copy of the project's .luarc.json,
with an explicit diagnostics.enable value. The real project configuration was not
edited. Both controls used the same wrapper, release binary, and request window 20.

| diagnostics.enable | Run 1 wall | Run 2 wall | LuaLS CPU during symbol span |
|---|---:|---:|---:|
| true | 12.573 s | 12.375 s | 11.90 / 11.71 s |
| false | 4.502 s | 2.837 s | 3.62 / 2.45 s |

All reference matches were identical to baseline. Disabled runs emitted no diagnostic
notifications and reused the same daemon/server PID. Thus file-open-triggered
background diagnostics explain much of the remaining cost; symbol generation, parsing,
and transport still remain. This experiment does not establish that disabling
server diagnostics is a generally acceptable behavior change.

Installed source confirms the trigger:
- libexec/script/provider/provider.lua:271 handles didOpen, files.open, and compileState.
- libexec/script/provider/diagnostic.lua:678 watches file events; the open branch calls
  doDiagnostic when the workspace is ready.
- libexec/script/provider/provider.lua:825 handles documentSymbol and converts all
  returned symbols; requests still require file-wide symbol generation.

## Daemon reuse defect

The trace initially showed a new daemon and server on each command. Capturing daemon
stderr in a separate foreground-managed experiment reproduced:

    failed to write daemon client message: failed to write JSON-RPC message: Broken pipe (os error 32)

The query itself succeeded. The daemon exited with status 1 and left a stale socket.
The coordinator drains upstream traffic before client events, and downstream write
errors propagate out of serve. Notifications racing with client disconnect can thus
terminate the daemon. This is intermittent: a manually managed daemon also survived
queries that still took 12.9/12.7 seconds, so lost reuse is a separate contributor,
not a complete explanation of the 13-second latency. For a fix, use a fake-server
regression that sends a notification after downstream disconnect and then verifies
that another client can use the same upstream server.

## Implications and tradeoffs

- Fixing disconnect handling should preserve reuse without changing query semantics,
  but does not remove diagnostics triggered by opening every file again.
- Reducing file opens or caching discovery could avoid more work, but requires an
  explicit completeness/invalidation design and server compatibility checks.
- Explicit user-selected LuaLS diagnostic settings give a concrete speed tradeoff:
  faster query-only sessions at the cost of diagnostic availability in that session.
  Do not hardcode Lua-specific configuration into generic lsp-cli logic.
- Runtime code and normal project/user configuration were not changed here.

## Artifacts from this session

- /tmp/lsp-release-bench-0z8zssym/results.json: uninstrumented release baseline.
- /tmp/lsp-release-trace-ab66lhav/{wire.jsonl,runs.json}: normal-config profile.
  The wire log also has later foreground-daemon observations; filter by run timestamps.
- /tmp/lsp-diag-control-g7gfa7nw/{wire.jsonl,runs.json}: diagnostics enabled.
- /tmp/lsp-diag-control-h7mzmx60/{wire.jsonl,runs.json}: diagnostics disabled.
- /tmp/lsp-proxy-daemon-exit-v2jzda7x/daemon.stderr: captured broken-pipe failure.
- Benchmark and reproducer scripts: /tmp/lsp-cli-release-benchmark.py,
  /tmp/lsp-cli-release-trace.py, /tmp/lsp-lua-trace-proxy.py,
  /tmp/lsp-cli-diagnostics-control.py, /tmp/lsp-cli-proxy-daemon-exit-check.py.

# Encountered difficulties

## What confused me

Repeated commands were initially assumed to be warm. PID traces and captured stderr
showed intermittent daemon termination. The live project also gained five Lua files
since the earlier measurements; release/debug cross-series comparisons need that caveat.

## Where to report

If you're sure the reported difficulties above are related to techplatform (e.g. userver, c35),
please report to [aisuite](https://nda.ya.ru/t/EcUMOwSH7eudWX).
