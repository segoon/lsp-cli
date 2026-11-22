# Anonymous namespace moved from header to source

The user requested moving the anonymous namespace into a `.cpp`, committing the
change, and retrying the comparison. No clarification questions were asked.

The original coverage fixture had `01_user.cpp` include `client.hpp`. That header
contained an anonymous namespace with `struct HttpClient {};` and
`inline void consume_hidden(HttpClient*) {}`. The entire namespace now lives in
[`01_user.cpp`](fixtures/clangd-source-anonymous/01_user.cpp); the include and header
are unnecessary. The named-namespace control remains in
[`00_normal.cpp`](fixtures/clangd-source-anonymous/00_normal.cpp).
These files preserve both class declarations and both reference expressions.

## Comparison

Real clangd 23.1.0, background indexing enabled, result/reference caps disabled.
Both translation units have generated absolute-path compilation database entries.
Each strategy ran twice in a separate server session and passed the same exact
two-anchor, two-reference oracle, recorded independently in
[`expected.json`](fixtures/clangd-source-anonymous/expected.json).

| Strategy | Seconds, two runs | Document-symbol requests | Workspace-symbol requests | References |
| --- | --- | --- | --- | --- |
| Exhaustive, with readiness | 0.711 / 0.663 | 3 | 0 | 2 / 2 expected |
| Configured workspace discovery | 0.782 / 0.727 | 0 | 1 | 2 / 2 expected |

Both strategies opened two documents and issued two reference requests. Exhaustive
discovery includes one readiness priming outline in addition to the two-file scan.
Workspace discovery waits for `backgroundIndexProgress` completion before search;
its optional header-outline patterns match no files in this source-only fixture.
All runs exited successfully, had no protocol errors, and left input files intact.
The [full measurement records](clangd-source-anonymous-results.jsonl) retain exact
positions, returned matches, timing and request counts.

The unchanged comparison harness was `/tmp/clangd-validity/compare.py`, using
`run_case(label, workspace, 20, 10, truth_path=oracle, readiness=True)` for exhaustive
and `run_case(..., generic=True)` for configured workspace discovery. It used the
previously built `/tmp/clangd-validity/readiness-cli` and
`/tmp/clangd-generic-default/lsp-cli` experimental executables respectively. These
are experimental builds, not the production CLI in this checkout. Runtime inputs
were copied to `/tmp/clangd-generic-default/source-anonymous`; its compilation
database and URI-based oracle were generated there rather than committed with
checkout-specific paths. The committed oracle uses relative filenames and
one-based coordinates; raw LSP records use zero-based coordinates.

## Tradeoffs and limits

Moving the private declaration into a source file makes it discoverable through
clangd's workspace index and avoids header parsing for discovery. On this tiny
fixture it did not yield a speedup; startup/indexing overhead dominates.

Keeping the original header layout preserves its previous translation-unit-local
entities, but clangd deliberately excludes its anonymous declarations from the
index. Scanning header outlines finds those declarations at additional parse cost,
without proving complete references across translation units. A shared named
namespace would be another design, changing entity identity and API exposure.

This move is valid for this fixture's single consumer. It does not establish
workspace-search completeness for arbitrary code, remove the separate macro
counterexample, or enable a production fast-path policy.

## Validation difficulties

`make test` ran: 396 passed, one ignored, one failed. The existing unrelated
`sends_configured_ids_in_direct_and_windowed_document_opens` failure expects
`csharp` while the current catalog provides `cs`. No Rust/catalog changes are
included here. The first benchmark assertion expected only two exhaustive outline
requests; the trace showed the additional readiness priming request. The recorded
comparison correctly counts all three. Shell commands required execution outside
the failing `bwrap` sandbox.
