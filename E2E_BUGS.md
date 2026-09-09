# E2E: why real-server smoke cases have `exceptions`

## Mechanism

In `tests/e2e/manifest/query_case.rs`, a `smoke` pair can be `status: queries`,
and each query case carries an optional `exceptions` list. Each entry names a
`command` (one of the real-server query kinds — `grep`, `references`,
`callers`, `callees`, `build-index`, `format`, etc.), an `outcome` (`failure`
or `empty-matches`), an optional expected stderr `message`, and a mandatory
`reason`.

At runtime (`tests/e2e/real_servers.rs:112-130,303-338`), if a query has a
matching exception, the harness skips the normal "must succeed with real
matches" assertion and instead asserts the *documented* deviant behavior:

- `failure`: the command must exit non-zero and stderr must contain `message`.
- `empty-matches`: the command must succeed but return an empty `matches`
  array.

So `exceptions` is not error-tolerance or flakiness suppression — it's a
positive assertion of each server's known, reproducible protocol quirk, with
the `reason` pinned in the yaml so the deviation is self-documenting and any
regression still fails loudly.

## Root causes, grouped

1. **No background-indexing-completion signal (`build-index` → `failure`,
   message "background-work progress").** Several servers advertise
   `$/progress`/work-done tokens but never send a terminal "index build
   finished" notification the CLI can wait on: clangd (c, cpp, cuda, objc,
   objcpp), OmniSharp (cs), vtsls (js/ts), pyright (python), Zuban (python),
   typescript-language-server (js/ts). This is the single most common
   exception across the suite.

2. **Workspace-symbol search (`grep`) returns nothing before indexing
   finishes.** clangd, vtsls, pylyzer, pyright, EmmyLua all report empty
   `matches` for `workspace/symbol` queries issued immediately after startup,
   since none exposes a synchronous "ready" signal — the server hasn't
   indexed the workspace yet when the query fires.

3. ~~**ts_ls (typescript-language-server) "No Project" errors.**~~ **Fixed.**
   For both TS and JS, ts_ls used to throw `No Project` on `grep`,
   `references`, `callers`, `callees`, `definition`, `declaration` because
   these all called `workspace/symbol` before any document had been opened
   via `textDocument/didOpen`, and ts_ls only attaches a TS project on the
   first `didOpen`. This was a genuine CLI ordering bug, not just a server
   quirk to document around: `references`/`callers`/`callees`/`definition`/
   `declaration` already had a document-symbol-scan fallback
   (`exact_named_document_anchors` in `src/commands/symbol_query.rs`) that
   opens documents first, but the code called `workspace/symbol`
   unconditionally *before* trying that fallback. Reordering
   `select_named_anchors` to try the document-scan path first (falling back
   to `workspace/symbol` only when it finds nothing) fixed all five. `grep`
   has no document-scan alternative, so it now retries once — opening one
   workspace file to prime a project — if the first `workspace/symbol` call
   fails. Verified against live ts_ls (`E2E_CASES="typescript/ts_ls,javascript/ts_ls"`)
   and re-checked for regressions against rust_analyzer/gopls/pyright, which
   share the same code path.

4. **Call-hierarchy has no edges for the fixture.** ~~Go (`gopls`) and Rust
   (`rust_analyzer`) report empty `callees` for `SampleOrder`/`sample_order`
   because that fixture constructs data directly rather than calling other
   functions~~ **Fixed for Go/Rust** by extracting a `newItem`/`new_item`
   helper so the fixture has a real outgoing call; verified against live
   gopls/rust_analyzer. EmmyLua (lua) still has no outgoing edges despite a
   real same-file call existing in source (a genuine EmmyLua limitation, not
   fixable via fixture changes), and pylyzer (python) still has no incoming
   (`callers`) edges despite a real cross-file caller existing.

5. **Server-specific formatting/output bugs.** EmmyLua's `format` returns an
   edit whose range falls outside the requested file — a genuine bug in the
   server, tolerated as a `failure` exception with matched message
   ("returned a line outside").

## Related: servers excluded entirely (`status: excluded`)

These aren't `exceptions` entries but explain further gaps in coverage:

- `denols` (Deno LSP) rejects the standard shutdown request because it
  requires non-null parameters.
- `roslyn_ls` (cs) and `lua_ls` (lua) have lifecycle-level incompatibilities:
  no smoke queries at all, or no clean exit after direct shutdown.
- Several Python servers fail to launch/initialize correctly in the isolated
  harness: `pylsp` (Mason launcher can't import the module), `pyre` (same),
  `pyrefly` (initializes but returns no workspace/document symbols).

## Bottom line

The exceptions exist because real LSP servers deviate from the LSP spec's
strict guarantees in ways that are reproducible but server-specific — mainly
(a) no standard signal for "background indexing/build is done," (b) symbol
search racing ahead of indexing, and (c) a couple of servers requiring a
document to be opened before workspace-wide queries work. Rather than
weakening assertions globally, the suite encodes each deviation explicitly
per server/command so real regressions still fail, while known quirks are
pinned and self-documented via `reason`.
