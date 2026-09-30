# E2E LSP compatibility investigation and remediation plan

Status: Phase 0, expected-failure triage, and Phase 2 implementation/type-definition plus
caller/callee fixture repair complete; PerlNavigator definition targeting remains a TODO pending
a product decision about use-site selection. Cross-server changed-fixture verification and Rust
test-setup deduplication are complete.

This document records the current E2E compatibility findings and prepares the work needed to
reduce exceptions, expected failures, and exclusions. Counts are derived from
`tests/e2e/cases/suite.yaml` and the language case files in `tests/e2e/cases/`. They describe the
currently pinned `lsp-cli-data` revision and will drift when that revision changes.

## Executive summary

The E2E inventory contains 358 distinct LSP server IDs. Of these, 274 appear in at least one of
the following categories:

- 16 servers have one or more explicit query exceptions;
- 83 servers have one or more expected-failure cases;
- 177 servers are excluded from at least one provisioning or executable-test scope.

These sets overlap:

- `pylyzer` has both a query exception and an expected failure;
- `salt_ls` has both an expected failure and an exclusion;
- no server appears in all three categories.

Consequently, 84 of the 358 servers have none of these classifications.

The 274 servers must not be treated as 274 confirmed server defects. Most are coverage or product
policy exclusions. All remaining expected failures are now diagnosed against a pinned registry
snapshot.

## Meaning of each classification

### Query exception

The test executes and positively asserts one documented deviant result. Depending on the entry,
the command must fail with a matched message, succeed with no matches, or succeed with a variable
number of matches. A different result still fails the test.

### Expected failure

The complete case executes, but any failure is removed from the target's fatal failure list. A
passing marked case is reported for review. The current marker does not constrain the failure
stage, error class, or message, so an unrelated infrastructure failure can satisfy it.

### Exclusion

The excluded scope does not execute and must carry a reason. Exclusion can apply only to
provisioning, smoke queries, lifecycle scenarios, or direct `run`; it does not necessarily exclude
the server from every E2E activity.

## Findings

### Provisioning exclusions: 160 servers

These classifications are mutually exclusive:

| Cause | Servers | Interpretation |
| --- | ---: | --- |
| No package in the current Mason registry | 90 | lsp-cli has no Mason recipe to execute |
| Outside the current general-purpose-server E2E scope | 55 | Specialized linter, formatter, framework server, adapter, or authenticated service |
| Unsupported installation mechanism | 11 | Four RubyGems, three LuaRocks, and one each of Open VSX, OPAM, Composer, and a source-build recipe |
| Obsolete or replaced | 3 | The manifest deliberately avoids a deprecated/discontinued server |
| Platform/toolchain unavailable | 1 | `sourcekit` is not provisionable in the Linux lane |

These entries are primarily provisioning or scope gaps, not evidence of LSP protocol failure.

### Pair-level exclusions: 17 servers

These 17 servers do not overlap the 160 provisioning exclusions.

| Cause | Servers | Examples |
| --- | ---: | --- |
| Shutdown or process-lifecycle incompatibility | 6 | `denols`, `jdtls`, `jedi_language_server`, `kotlin_language_server`, `lua_ls`, `robotcode` |
| Installation/startup prerequisite or upstream packaging failure | 5 | `arduino_language_server`, `cmake`, `kotlin_lsp`, `rpmspec`, `salt_ls` |
| Missing generic initialization configuration | 1 | `astro` requires a TypeScript SDK path in `initializationOptions` |
| Shared semantic-profile mismatch or unstable results | 5 | `pylsp`, `pyre`, `pyrefly`, `roslyn_ls`, `ty` |

Important details:

- Deno rejects the standard parameterless `shutdown` request by demanding non-null parameters.
- Several servers answer requests but do not exit after the standard shutdown/exit exchange.
- `cmake-language-server` installs an incompatible current pygls dependency and fails at startup.
- Kotlin LSP's pinned Mason build reports that it has expired.
- Arduino Language Server needs board-specific external configuration and tools.
- Some Python servers initialize but do not expose enough discoverable semantic information for
  the shared source-language query profile.
- Roslyn returns decorated or qualified symbol names that the shared exact-name assertion does not
  accept.

### Query exceptions: 29 entries on 16 servers

| Cause | Exception entries | Assessment |
| --- | ---: | --- |
| No usable background-index completion signal | 18 | Protocol/product-semantics problem affecting 12 servers |
| Fixture/profile requests an inapplicable relationship | 2 | Test-fixture/profile problem, not a server failure |
| Workspace-symbol readiness race | 3 | Generic readiness problem |
| Missing semantic result despite an applicable query | 4 | Server, fixture, or capability-advertisement limitation |
| Invalid formatting edit | 1 | EmmyLua returns an edit outside the requested file |
| Nondeterministic definition cardinality | 1 | PerlNavigator varies between no result and the declaration itself |

The two remaining fixture/profile exceptions are the clearest low-risk cleanup candidates. C
cannot provide an implementation relationship without changing language semantics; Perl's
definition query should move from its declaration to a stable use site.

The 18 `build-index` entries require a product decision. LSP has no universal notification meaning
"the whole workspace is indexed." A generic implementation cannot promise confirmed completion
for a server that exposes no terminal progress signal.

### Expected failures: 154 cases on 83 servers

| Phase | Cases | Distinct servers |
| --- | ---: | ---: |
| Capabilities | 118 | 81 |
| Provisioning | 35 | 35 |
| Smoke queries | 1 | 1 |

There are 34 servers shared by the capability and provisioning groups. The remaining smoke server
is not shared with those groups, producing 83 distinct server IDs overall.

All 35 provisioning, 118 capability, and one smoke-query entry identify their pinned package
source and concrete observed cause. Four stale provisioning markers (`marko-js`, `mesonlsp`,
`millet`, and `terraformls`) and the stale OmniSharp, EmmyLua, and BasedPyright smoke markers were
removed after two fresh pinned-snapshot passes apiece. The generic markers were originally added by
commit `9fcb724` together with expected-failure runner support.

### Pinned provisioning result

Snapshot `2026-09-30-aboard-mob`
(`sha256:69a52e9d625c790d4811b8a1295df09e681d3faff1c09a80909b661773d127f9`)
executed all 198 provisioning cases with two workers:

- 163 passed;
- 35 expected failures remained;
- no unexpected failures or registry rate-limit errors occurred;
- npm passed 47/47 and NuGet passed 2/2;
- the main shared implementation gap is discarded Cargo source qualifiers (`repository_url`,
  `rev`, `locked`, and `features`), affecting six retained failures;
- four Cargo/PyPI subprocess failures retain only a trailing help/note line, so their deeper cause
  remains unknown until installer error reporting preserves actionable stderr.

### Pinned capability result

The same snapshot executed all 118 marked capability cases with two workers. All 118 failed again;
there were no stale passing markers. The failures are mutually classified by the first failing
stage or concrete root cause:

| Cause | Cases | Servers |
| --- | ---: | ---: |
| Provisioning failure propagated into the capability case | 39 | 26 |
| Server did not exit after the standard shutdown/exit exchange | 29 | 23 |
| Literal placeholder or unresolved launcher path | 13 | 7 |
| Missing or incompatible host runtime/tool | 12 | 5 |
| Shutdown response incompatibility or premature transport close | 9 | 6 |
| Malformed initialize response or transport framing | 7 | 5 |
| Required project/server configuration or external tool | 4 | 4 |
| Upstream release incompatibility or external service dependency | 3 | 3 |
| Initialize timeout | 1 | 1 |
| Unexplained close during initialize | 1 | 1 |

The raw per-case reports and logs are retained in
`target/e2e-capability-results.Cgsq10`. The snapshot digest is
`sha256:69a52e9d625c790d4811b8a1295df09e681d3faff1c09a80909b661773d127f9`.

The largest product-level opportunity is generic bounded cleanup after a successful shutdown
exchange, but it can address only the 29 post-`exit` hangs. It must not hide the nine cases that
reject shutdown or close before replying. Placeholder configurations and required initialization
options belong in the data/configuration layer; server-specific production branches would violate
the language-neutral architecture.

### Pinned smoke-query result

The same snapshot executed all four marked smoke cases in fresh isolated homes:

- `cs/omnisharp` passed the complete query suite twice, so its stale marker was removed;
- `lua/emmylua_ls` reproducibly reached `textDocument/implementation` and returned no match for
  the plain `format_timestamp` function after three bounded attempts;
- `python/basedpyright` reproducibly returned no implementation match for the plain
  `build_sample_order` function after three bounded attempts;
- `python/pylyzer` passed the implementation query, then returned no type-definition match for
  `build_sample_order` after three attempts. It also logged a missing `ERG_PATH` and repeated
  diagnostics-worker panics, but those were not the direct failed assertion.

The original three failures were shared-fixture/profile mismatches: the profile requested
implementation or type-definition relationships that the plain function fixtures did not create.
After command-specific query targets and genuine fixture relationships were added, EmmyLua and
BasedPyright passed twice. Pylyzer still returns no type definition for the same explicitly typed
parameter that passes with BasedPyright and pyright, so its retained marker is now server-specific.
The raw reports are retained in `target/e2e-smoke-triage.b59ZRP`.

### Limitations of the available full-run log

The untracked `e2e.log` cannot be used as a server-by-server diagnosis:

- the run planned 511 cases and executed 495;
- 46 passed, 291 unmarked cases failed, 158 marked cases failed, and 3 marked cases passed;
- 279 of the 291 detailed unmarked failure blocks explicitly report GitHub/Mason HTTP 403 rate
  limiting;
- the other 12 detailed blocks are lifecycle failures with no installation receipt after the
  provisioning phase failed, so they appear to be downstream effects of missing installations;
- expected-failure diagnostics are filtered out of the final detailed failure report, so the log
  cannot establish why the 158 marked cases failed;
- `provisioning/marko-js`, `provisioning/mesonlsp`, and `provisioning/millet` passed despite their
  expected-failure markers.

The run demonstrates an infrastructure incident, not hundreds of independent server failures.
Individual expected-failure markers must be reproduced with a pinned registry snapshot before
their causes are trusted.

## Remediation plan

### Phase 0: establish trustworthy evidence

1. Generate one authenticated Mason registry snapshot with the existing
   `e2e-workflow-plan` binary.
2. Pass that snapshot through `E2E_MASON_REGISTRY_SNAPSHOT` to every isolated E2E case, matching
   the existing CI workflow.
3. Run provisioning first, grouped by installation family and at conservative concurrency.
4. Run capability and smoke cases only after their server's provisioning result is known.
5. Preserve structured per-case diagnostics for expected failures instead of reporting only their
   count.
6. Record package source IDs and the registry snapshot digest with every diagnosis.

Pros:

- removes the dominant shared failure source;
- makes package versions and results reproducible;
- prevents work on false server-specific diagnoses.

Cons:

- requires authenticated registry access;
- a complete real-server run is expensive in time, bandwidth, CPU, and storage;
- upstream package availability can still change when a new snapshot is deliberately selected.

Alternative: reproduce one server at a time without a shared snapshot. This is simpler to start,
but it is slower overall, can hit the same rate limit, and does not guarantee that cases use the
same registry state.

### Phase 1: triage expected failures

Process cases in this order:

1. ~~the original 39 provisioning cases~~ — complete; 35 remain and four stale markers were
   removed;
2. ~~the 118 capability cases, grouped by server so shared failures are diagnosed once~~ —
   complete; all 118 remain with concrete reasons;
3. ~~the four smoke cases~~ — complete; three fixture/profile failures remain and OmniSharp's stale
   marker was removed;
4. ~~repeat the passing provisioning markers before removal~~ — complete.

For each case, assign one concrete class:

- registry/package unavailable;
- unsupported installer backend or recipe;
- missing host runtime/program;
- launcher or executable-resolution defect;
- server startup failure;
- initialization rejection or timeout;
- malformed LSP transport;
- shutdown/process-cleanup failure;
- harness or fixture defect;
- transient infrastructure failure.

Remove a marker after a repeatable pass. Otherwise, replace its generic reason with the observed
source ID, failure stage, user-visible diagnostic, and root-cause category.

Recommended future hardening: expected failures should optionally match a stage and diagnostic
substring, as query exceptions already do.

Pros:

- prevents an unrelated registry outage from satisfying a server-specific expected failure;
- turns the manifest into actionable compatibility documentation.

Cons:

- upstream wording changes can require maintenance;
- overly exact matching can create brittle tests.

Alternative: remove expected-failure tolerance entirely. This maximizes strictness but would make
the exhaustive target unusable while the backlog remains untriaged. Another alternative is a
separate quarantine lane; it keeps the main target strict but increases CI and reporting
complexity.

### Phase 2: remove fixture-caused query exceptions

1. ~~Change fixture callables so implementation/type-definition tests point at symbols for which
   those relationships genuinely exist.~~ Complete: a backward-compatible `command-queries` map
   selects relationship-specific symbols; nine misleading exceptions and two broad smoke markers
   were removed. C's implementation query is inherently inapplicable, while rust-analyzer and
   pylyzer retain evidence-backed server limitations against genuine type relationships.
2. ~~Add explicit named caller/callee edges rather than relying on constructors or top-level
   code.~~ Complete: Clojure and Luau now query functions with named incoming and outgoing edges,
   removing two fixture-caused exceptions. EmmyLua still returns no callees for direct named
   function and method calls, so its exception remains as a verified server limitation.
3. **TODO:** Move PerlNavigator's definition query from a declaration to a stable use site. The
   current name-only command discovers declaration symbols, PerlNavigator does not support
   `textDocument/references`, and it returns no definition for every tested declaration. Completing
   this requires a product decision between generic textual use-site discovery, a position-based
   public CLI option, or retaining the documented exception.
4. ~~Verify each changed fixture against every server sharing that language fixture.~~ Complete:
   all 13 executable smoke pairs were rerun against the final fixtures with no unexpected failures;
   six other shared-fixture pairs remain intentionally excluded from semantic smoke execution.
5. ~~Deduplicate setup when adding or changing Rust E2E tests.~~ Complete: expected-failure tests
   reuse the manifest mutation/validation helper instead of repeating full-manifest setup. The
   remaining helpers are domain-specific and combining them would reduce readability.

Phase 2 item 1 validation against Mason snapshot `2026-09-30-aboard-mob`:

- every affected executable pair passed targeted real-server validation; Objective-C++/clangd
  additionally passed three consecutive smoke runs after its relationship anchor moved into a
  scanned `.mm` source file;
- the complete 511-case suite executed twice. The first run exposed the Objective-C++ background
  index race plus two transient Solidity download/startup failures. After the fixture correction,
  every changed semantic case passed in the second complete run, including C, Clojure, C++, CUDA,
  Lua, Objective-C, Objective-C++, Python, and Rust;
- the second aggregate run did not exit successfully because 12 unrelated cases failed: ten
  package download/provisioning requests, OmniSharp daemon reuse, and Verible shutdown. Those are
  infrastructure/lifecycle failures outside this item; the first complete run had passed the
  affected OmniSharp, download-backed, and Verible cases, so they are not attributed to the query
  profile or fixture changes.

Phase 2 item 2 validation against the same snapshot:

- Clojure and Luau passed twice with named incoming and outgoing fixture edges and without their
  former caller/callee exceptions;
- EmmyLua returned no callees for both a direct named local-function edge and a concrete annotated
  method edge. Its smoke case passes with the retained exception narrowed to that server behavior;
- the query-exception inventory fell from 31 entries to 29 while remaining on 16 servers;
- `make -j6 test-e2e` planned 511 cases and executed 495. Every changed case passed, but the
  aggregate run failed on seven unrelated cases: four GLSL Analyzer and one Verible shutdown
  broken pipe, OmniSharp returning different build-index failure text, and Terraform LS's upstream
  package URL returning HTTP 404. The run reported 334 ordinary passes, 154 expected failures, and
  16 exclusions.

Phase 2 item 4 validation against the same snapshot:

- the final fixture state passed targeted smoke validation for C/clangd, Clojure/clojure-lsp,
  C++/clangd, CUDA/clangd, Lua/EmmyLua, Luau/luau-lsp, Objective-C/clangd,
  Objective-C++/clangd, Python/BasedPyright, Python/pyright, Python/Zuban, and
  Rust/rust-analyzer;
- Python/pylyzer reproduced its declared expected failure with no unexpected runner failure;
- Lua/LuaLS and Python/Jedi Language Server, pylsp, Pyre, Pyrefly, and ty remain intentionally
  excluded from semantic smoke execution for their documented lifecycle or semantic-capability
  limitations. The audit does not misrepresent those exclusions as successful fixture validation.

Phase 2 item 5 validation:

- the whole-file audit covered the manifest, command-query, expected-failure, and real-server test
  modules;
- `make test` passed 341 tests with one ignored test plus all 82 E2E-runner tests;
- Clippy passed for all targets and features with warnings denied;
- the change only deduplicates test setup and does not alter the E2E runner, manifest schema, suite
  selection, phases, Make targets, or CI, so no additional real-server suite run was required.

Pros:

- removes misleading server exceptions without weakening production assertions;
- keeps production code language-neutral.

Cons:

- a fixture improvement for one server can change results returned by another compatible server;
- some languages do not naturally support every relationship, so capability-only expectations may
  still be appropriate.

Alternative: extend the test expectation schema to declare a relationship as legitimately empty.
This is more honest than calling it a server exception but provides less semantic coverage than a
fixture containing a real relationship.

### Phase 3: improve generic readiness and lifecycle handling

Investigate language-neutral changes only:

- prime semantic queries with `textDocument/didOpen` and bounded LSP-visible readiness signals;
- use progress or diagnostics notifications when available, without assuming they prove complete
  workspace indexing;
- after a successful shutdown response and `exit` notification, apply a bounded grace period and
  consider controlled process termination;
- keep Deno's invalid shutdown-parameter requirement separate from ordinary exit hangs;
- add generic, data-driven initialization options if the product should support Astro-like
  servers.

Pros:

- one generic improvement can remove several server-specific exclusions;
- bounded behavior improves command-line predictability.

Cons:

- no generic LSP signal proves complete indexing for every server;
- forced termination can hide upstream lifecycle defects or discard server cleanup work;
- data-driven initialization options expand trusted configuration and validation surface.

Alternative for lifecycle failures: retain strict shutdown semantics and exclusions. This preserves
the strongest correctness signal but leaves otherwise useful servers unavailable to direct mode.

Alternative for initialization options: expose only a user-supplied CLI/config value. This avoids
embedding server behavior in the data catalog but makes automatic detection and E2E provisioning
less self-contained. Hardcoding Astro or another server in production is not acceptable.

### Phase 4: repair upstream/package-specific exclusions

Prioritize fixes that do not require new product architecture:

- update or constrain the `cmake-language-server`/pygls combination;
- select a non-expired Kotlin LSP build when available;
- verify `salt-lsp` against the managed Python runtime;
- determine whether compatible RPM Python bindings can be staged hermetically;
- decide whether Arduino's board core, CLI configuration, clangd, and FQBN belong in a dedicated
  integration fixture;
- normalize only E2E expectations, not production LSP results, for decorated Roslyn symbol names.

Pros:

- fixes concrete cases without weakening shared behavior.

Cons:

- version pins and upstream workarounds require ongoing maintenance;
- system bindings and board toolchains substantially increase CI setup cost.

Alternative: retain reviewed exclusions until upstream packages become self-contained. This keeps
the harness maintainable but provides no executable compatibility guarantee.

### Phase 5: decide provisioning and coverage strategy

The following are product-owner decisions and must be resolved before broad implementation:

1. Should the 55 specialized servers remain outside the real-server scope, or receive
   capability-only coverage?
2. Should lsp-cli implement RubyGems, LuaRocks, Open VSX, OPAM, Composer, and source-build Mason
   recipes, rely on system-installed binaries for those servers, or retain exclusions?
3. For the 90 servers absent from Mason, should the project contribute Mason packages, support an
   additional registry, add a non-download E2E lane, or accept that automatic installation is not
   covered?
4. Should `build-index` promise confirmed completion, return best-effort readiness when the server
   exposes no completion signal, or report a user-facing unsupported-completion error?
5. Should direct commands tolerate and terminate a server that completed the LSP shutdown exchange
   but did not exit by itself?

Strategic alternatives:

- **Add installer families.** Improves automatic-download coverage but expands security surface,
  host-runtime requirements, cache formats, tests, and CI matrices.
- **Use a system-installed-server lane.** Requires less installer code but is less hermetic and
  harder to reproduce.
- **Support another registry.** Can address servers missing from Mason, but creates a major
  selection, trust, versioning, and maintenance commitment.
- **Contribute upstream Mason packages.** Keeps lsp-cli simpler and preserves one registry, but
  depends on external review and maintenance.
- **Keep exclusions.** Lowest maintenance cost, but the catalog will continue to overstate
  executable/downloadable coverage unless user-facing documentation distinguishes the levels
  clearly.

## Architectural consequences and future risks

- Adding installer backends affects the core download trust boundary and requires archive/path,
  executable-resolution, receipt, cache, and runtime tests for each family.
- Generic initialization options make the data catalog responsible for behavioral launch
  configuration, not just discovery metadata. Schema validation and trust expectations must be
  explicit.
- A tolerant shutdown policy changes user-visible success semantics. It should distinguish a
  completed LSP exchange from an unknown or interrupted server state.
- Best-effort `build-index` would weaken the meaning of command success; strict completion leaves
  many conforming servers unsupported because LSP lacks the necessary universal signal.
- Expanding scope to linters, formatters, and framework servers changes what "supported LSP
  server" means and may require capability-specific test profiles rather than one general-purpose
  semantic profile.
- Broad expected-failure markers can hide regressions unless they match an expected failure class.
- Server and registry versions drift. Every retained exception or exclusion should identify the
  tested source ID or snapshot when practical.

## Validation requirements for later implementation

No validation command is required for this documentation-only addition. For subsequent changes:

- run focused unit tests while developing;
- add regression tests for any harness or product defect;
- keep test setup compact and deduplicated;
- validate changed behavior against the relevant committed playground projects;
- after any E2E infrastructure, runner, manifest-schema, suite-selection, phase, Make target, or CI
  workflow change, run the complete real-server suite with `make test-e2e` using a pinned,
  authenticated registry snapshot;
- also run the repository-wide `make test` before completion.

## Current constraints and investigation difficulties

- The only available exhaustive-run log was contaminated by GitHub rate limiting.
- Expected-failure diagnostics were not retained in the old exhaustive log; the dedicated pinned
  runs now retain structured provisioning and capability evidence.
- The original generic reasons could not be recovered; all remaining markers have now been
  reproduced against the pinned snapshot and replaced with concrete evidence.
- Upstream package/server behavior may have changed since the pinned snapshot and must be verified
  before removing an exclusion.
- Capability triage established new malformed-response, configuration, runtime, and lifecycle
  gotchas; they are recorded in `docs/GOTCHAS.md`.

No product or E2E-runner implementation has been changed by the triage. Manifest reasons and
compatibility documentation now preserve the reproduced evidence.
