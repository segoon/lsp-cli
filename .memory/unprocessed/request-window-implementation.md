# Request-window implementation decisions and validation (2026-09-05)

The user requested a default of 20 concurrent per-file requests, configurable as
`max-requests-in-flight` in lsp-cli.yaml. During planning the user selected:
- Named queries only (references, definition, declaration, callers, callees).
- Fail on document-symbol timeout instead of silently skipping it.

The implementation uses bounded document-symbol scheduling in the client, matches
response IDs, decodes immediately, and preserves scan order. Synchronous request
transmission is shared. Do not invoke the unbounded notification-drain helper inside
window scheduling: continuous traffic could prevent deadline checks. Regression tests
exercise continuous notifications, refill with an older outstanding request, reversed
responses, server requests, cancellation, and deterministic named-query results.

Debug-build benchmark, without verbose logging, with separate temporary configuration
and daemon runtime directories for each limit:
- parley.nvim, limit 1: 20.012 s cold / 22.382 s warm.
- parley.nvim, limit 20: 13.519 s cold / 13.200 s warm.
- Lua playground, limit 1: 1.139 s cold / 0.503 s warm.
- Lua playground, limit 20: 1.148 s cold / 0.352 s warm.
All four outputs for each workspace were identical. These are two measurements per
limit, not a statistical performance guarantee. Artifacts are in
/tmp/lsp-window-bench-7mvr24xb/results.json for this session.

A direct-process fake server also verified the default window, 23 file requests,
reversed responses, all 23 anchors, and clean shutdown. Its temporary script and
fixture are /tmp/lsp-cli-window-stdio-check.py and /tmp/lsp-window-stdio-msmp85nt.
