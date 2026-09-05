# References query performance investigation (2026-09-05)

User requested investigation of a 17.715-second detached references query for
`normalize_timestamp` in `/home/segoon/projects/parley.nvim`.

Installed `/home/segoon/.cargo/bin/lsp-cli` reproduced the same result in 19.744 and
22.107 seconds without debug logging. A timestamped debug run took 20.303 seconds:
initialize 0.076 s, workspace/symbol 0.040 s, 188 sequential documentSymbol requests
17.368 s combined, references 0.063 s, shutdown 0.024 s. Debug logging adds overhead,
so these are phase measurements, not a controlled estimate of every source of latency.

`select_named_anchors` always scans all matching documents when document symbols are
supported. It is not merely a fallback for empty workspace-symbol results. Here
workspace/symbol returned null, so preferring workspace results alone would not help.
The scanned files were 100 under lua/, 86 under tests/, one plugin and one script.
Only lua/parley/timestamp.lua contains the literal query (checked with rg).

Daemon::serve sleeps 25 ms after each iteration, draining upstream before downstream.
This adds latency to sequential exchanges, independently of server computation.
--limit truncates final output and does not bound discovery work.

Direct runs used an isolated XDG_RUNTIME_DIR because connect_lsp_client reuses existing
sockets even with --no-detach. They took 45.521 s with debug and 44.738 s without:
query work finished, but server exit timed out after another 30 seconds. This is a
separate unresolved behavior, documented in GOTCHAS.md, not a successful workaround.

Potential changes: event-driven daemon wakeups preserve discovery semantics but need
transport work; literal source prefiltering greatly reduces candidates here but could
miss server-provided names absent literally from source. No runtime code was changed.
Future regressions should exercise delayed/local-symbol discovery and verify protocol
request counts; daemon latency checks should use a fake immediate-response server.
Timestamped traces and result files are in /tmp/lsp-cli-{detached,direct}* for this session.
