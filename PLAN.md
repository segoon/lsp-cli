# E2E LSP compatibility investigation and remediation plan

Status: Phase 0 through Phase 8 are complete.
PerlNavigator definition targeting remains a TODO pending a product decision about use-site
selection. Cross-server changed-fixture verification and Rust test-setup deduplication are
complete.

This document records the current E2E compatibility findings and prepares the work needed to
reduce exceptions, expected failures, and exclusions. Counts are derived from
`tests/e2e/cases/suite.yaml` and the language case files in `tests/e2e/cases/`. They describe the
currently pinned `lsp-cli-data` revision and will drift when that revision changes.

## Executive summary

The E2E inventory contains 358 distinct LSP server IDs. Of these, 209 appear in at least one of
the following categories:

- 9 servers have one or more explicit query exceptions;
- 83 servers have one or more expected-failure cases;
- 119 servers are excluded from at least one provisioning or executable-test scope.

These sets overlap:

- `pylyzer` has both a query exception and an expected failure;
- `salt_ls` has both an expected failure and an exclusion;
- no server appears in all three categories.

Consequently, 149 of the 358 servers have none of these classifications.

The 209 servers must not be treated as 209 confirmed server defects. Most are coverage or product
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

### Provisioning exclusions: 107 servers

These classifications are mutually exclusive:

| Cause | Servers | Interpretation |
| --- | ---: | --- |
| No package in the current Mason registry | 90 | lsp-cli has no Mason recipe to execute |
| Unsupported installation mechanism | 13 | Five RubyGems, three LuaRocks, two Open VSX, and one each of OPAM, Composer, and a source-build recipe |
| Obsolete or replaced | 3 | The manifest deliberately avoids a deprecated/discontinued server |
| Platform/toolchain unavailable | 1 | `sourcekit` is not provisionable in the Linux lane |

These entries are primarily provisioning or scope gaps, not evidence of LSP protocol failure.

### Pair-level exclusions: 12 servers

These 12 servers do not overlap the 107 provisioning exclusions.

| Cause | Servers | Examples |
| --- | ---: | --- |
| Shutdown or process-lifecycle incompatibility | 3 | `denols`, `jdtls`, `kotlin_language_server` |
| Installation/startup prerequisite or upstream packaging failure | 3 | `arduino_language_server`, `rpmspec`, `salt_ls` |
| Missing generic initialization configuration | 1 | `astro` requires a TypeScript SDK path in `initializationOptions` |
| Shared semantic-profile mismatch or unstable results | 5 | `kotlin_lsp`, `pylsp`, `pyre`, `pyrefly`, `ty` |

Important details:

- Deno rejects the standard parameterless `shutdown` request by demanding non-null parameters.
- Deno rejects shutdown, while jdtls and Kotlin Language Server retain distinct direct-lifecycle
  failures that occur after the exchange.
- Kotlin LSP's current Mason build initializes and shuts down cleanly, but returns no workspace
  symbols or references for the shared fixture even after its background-work progress completes.
- Arduino Language Server needs board-specific external configuration and tools.
- Some Python servers initialize but do not expose enough discoverable semantic information for
  the shared source-language query profile.
- Roslyn's decorated names are now asserted through pair-local E2E expectations without changing
  production results.

### Query exceptions: 16 entries on 9 servers

| Cause | Exception entries | Assessment |
| --- | ---: | --- |
| Fixture/profile requests an inapplicable relationship | 3 | Test-fixture/profile problem, not a server failure |
| Workspace-symbol readiness race | 4 | Generic readiness problem |
| Missing semantic result despite an applicable query | 7 | Server, fixture, or capability-advertisement limitation |
| Invalid formatting edit | 1 | EmmyLua returns an edit outside the requested file |
| Nondeterministic definition cardinality | 1 | PerlNavigator varies between no result and the declaration itself |

The three fixture/profile exceptions include inapplicable C implementation and Roslyn method
type-definition relationships. Perl's definition query remains the low-risk cleanup candidate and
should move from its declaration to a stable use site.

The former 21 `build-index` exceptions across 15 servers were removed in Phase 6. Those servers now
use bounded best-effort completion selected by generic LSP data; this accepts only a missing
terminal signal, not transport, protocol, server-reported, or shutdown failures.

### Expected failures: 164 cases on 83 servers

| Phase | Cases | Distinct servers |
| --- | ---: | ---: |
| Capabilities | 112 | 81 |
| Provisioning | 51 | 51 |
| Smoke queries | 1 | 1 |

There are 34 servers shared by the capability and provisioning groups. The remaining smoke server
is not shared with those groups, producing 60 distinct server IDs overall.

All 35 provisioning, 89 capability, and one smoke-query entry identify their pinned package
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

Generic bounded cleanup after a successful shutdown exchange was implemented in Phase 3 and the
29 post-`exit`-hang markers were removed. The final full-suite run passed every affected case. The
policy deliberately does not hide the nine historical cases that reject shutdown or close before
replying. Placeholder configurations and required initialization options belong in the
data/configuration layer; server-specific production branches would violate the language-neutral
architecture.

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

Phase 8 implements optional expected-failure stage and diagnostic-substring matching.

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

- ~~Prime semantic queries with `textDocument/didOpen` and bounded LSP-visible readiness
  signals.~~ Complete for workspace-symbol queries: after an initially empty result, lsp-cli opens
  the first detected source and, when advertised, uses `textDocument/documentSymbol` as a
  best-effort server-processing barrier before the existing bounded workspace-symbol polling.
  Named semantic queries already open or scan their target documents.
- ~~Use progress or diagnostics notifications when available, without assuming they prove complete
  workspace indexing.~~ Complete for workspace-symbol retries: the existing 750 ms retry delay now
  waits for a target-document diagnostic, completed work-done progress item, or healthy quiescent
  server status and can retry early once. Later retries remain spaced, and only a non-empty
  `workspace/symbol` response establishes success.
- ~~After a successful shutdown response and `exit` notification, apply a bounded grace period and
  controlled process termination.~~ Complete: owned child processes receive up to one second (or
  the shorter user timeout) to exit, after which lsp-cli terminates and reaps them while preserving
  command success. Shutdown rejection and pre-response transport failure remain errors.
- keep Deno's invalid shutdown-parameter requirement separate from ordinary exit hangs;
- add generic, data-driven initialization options if the product should support Astro-like
  servers.

Phase 3 workspace-symbol readiness validation against Mason snapshot `2026-09-30-aboard-mob`:

- a public-CLI fake-server regression now withholds workspace symbols until it receives the
  document-symbol barrier, so the existing binary-level query test exercises the new behavior;
- targeted real-server smoke cases passed for Clojure/clojure-lsp, Lua/EmmyLua, and Odin/OLS;
  those servers still returned empty workspace-symbol results, so their evidence-backed
  exceptions remain;
- `make test` passed 341 tests with one ignored test plus all 82 E2E-runner tests, and Clippy
  passed for all targets and features with warnings denied;
- `make -j6 test-e2e` planned 511 cases and executed 495. All changed cases passed; the aggregate
  run reported 336 ordinary passes, 154 expected failures, and 16 exclusions, but failed on five
  unrelated cases: OmniSharp daemon reuse, Terraform LS's upstream package URL returning HTTP
  404, two GLSL Analyzer shutdown broken pipes, and one Verible shutdown broken pipe.

Phase 3 notification-hint validation against the same snapshot:

- unit coverage verifies the target-document diagnostic wait and preservation, completed-progress
  recognition, and healthy quiescent server-status recognition; the public-CLI fake-server query
  matrix also passed;
- `make test` passed 345 tests with one ignored test plus all 82 E2E-runner tests, and Clippy
  passed for all targets and features with warnings denied;
- focused `make -j6 test-e2e` runs passed all planned provisioning, semantic, and lifecycle cases
  for Clojure/clojure-lsp, Lua/EmmyLua, and Odin/OLS (eight executed cases total). Their documented
  empty workspace-symbol exceptions remain because a readiness hint deliberately does not assert
  index completion.

Phase 3 bounded-shutdown validation against the same snapshot:

- a process-level regression verifies that a server which acknowledges `shutdown`, receives
  `exit`, and remains alive is terminated and reaped without turning the completed LSP operation
  into a failure;
- all 29 capability markers caused only by post-`exit` hangs were removed across 23 servers; Deno
  and six shutdown-response/transport incompatibilities remain separate and unchanged;
- LuaLS and Jedi Language Server now run semantic smoke coverage with narrow `build-index`
  exceptions, while both RobotCode language aliases run capability coverage. jdtls remains
  excluded from shared semantic smoke because project-index readiness and decorated method names
  are independently unstable, and its raw direct exchange exits with status 1;
- `make test` passed 346 tests with one ignored test plus all 82 E2E-runner tests, and Clippy passed
  for all targets and features with warnings denied;
- the final `make -j10 test-e2e` planned 512 cases and executed 499, reporting 368 ordinary passes,
  125 expected failures, and 13 exclusions. Every changed case passed. Six unrelated cases failed:
  three Terraform LS requests hit the upstream 0.39.0 HTTP 404, one GLSL Analyzer and one Verible
  case hit their intermittent shutdown broken pipe, and CUDA/clangd exhausted its case deadline
  under the higher-concurrency run after passing the preceding full run.

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

- ~~Update or constrain the `cmake-language-server`/pygls combination.~~ Complete: LSP data can
  add generic Mason `extra_packages`, PyPI installs honor them, and CMake Language Server pins
  `pygls<2` until upstream publishes a compatible dependency declaration;
- ~~Select a non-expired Kotlin LSP build when available.~~ Complete: Mason now resolves
  `kotlin-lsp/v263.4702.0`; provisioning and direct lifecycle coverage are enabled. Semantic smoke
  remains excluded because the server returns neither workspace symbols nor references for the
  shared fixture after reporting background work complete;
- ~~Verify `salt-lsp` against the managed Python runtime.~~ Complete: version 0.0.1 requires
  `PyYAML>=5.4,<6`; Python 3.12 has no compatible wheel, and the PyYAML 5.4.1 source build fails.
  The reviewed provisioning expected failure and semantic exclusion remain;
- ~~Determine whether compatible RPM Python bindings can be staged hermetically.~~ Complete: the
  PyPI `rpm` package is only a shim for OS-provided native bindings. Staging distro binaries would
  couple Python and native-library ABIs; upstream container mode requires a new runtime and TCP
  backend. The reviewed exclusion remains pending an explicit architecture decision;
- ~~Decide whether Arduino's board core, CLI configuration, clangd, and FQBN belong in a dedicated
  integration fixture.~~ Complete: they form one coupled, project-specific toolchain and should
  use a dedicated managed fixture, not production LSP data. Implementation awaits product approval
  for its additional downloads and setup orchestration; the reviewed exclusion remains;
- ~~Normalize only E2E expectations, not production LSP results, for decorated Roslyn symbol
  names.~~ Complete: schema v13 supports pair-local callable and expected-name overrides; Roslyn
  uses exact decorated strings plus six asserted deviations. `make -j10 test-e2e
  CASE=cs/roslyn_ls` passed provisioning, semantic queries, and lifecycle.

Phase 4 CMake validation covered same-transaction extra constraints, cache invalidation, and
configuration propagation without a server-specific branch. `make -j10 test-e2e CASE=cmake/cmake`
passed provisioning and capability exchange, removing the former startup exclusion.

Phase 4 Kotlin LSP validation: Mason resolved `kotlin-lsp/v263.4702.0`, replacing the expired
build. `make -j10 test-e2e CASE=kotlin/kotlin_lsp` passed provisioning and direct lifecycle;
semantic smoke remains excluded after longer work-done waits still produced no symbols or
references. No Kotlin-specific production behavior or broad timing relaxation was retained.

Pros:

- fixes concrete cases without weakening shared behavior.

Cons:

- version pins and upstream workarounds require ongoing maintenance;
- system bindings and board toolchains substantially increase CI setup cost.

Alternative: retain reviewed exclusions until upstream packages become self-contained. This keeps
the harness maintainable but provides no executable compatibility guarantee.

### Phase 5: decide provisioning and coverage strategy

The product owner resolved the coverage boundary:

1. The 55 initially identified specialized servers should receive capability-only coverage, not
   the shared semantic query profile, when their installer family is supported.
2. RubyGems, LuaRocks, Open VSX, OPAM, Composer, and source-build Mason recipes remain excluded.
   The affected servers are listed in `docs/SERVERS.md`, and backend work is deferred in
   `docs/TODO.md`.
3. The 90 servers absent from Mason remain not automatically installable. lsp-cli will not add a
   second registry or a system-installed E2E lane as part of this plan.
4. The 15 servers proven to lack a usable completion signal should use explicit, data-driven
   best-effort `build-index` semantics. Their exhaustive list is in `docs/SERVERS.md`; all other
   servers retain confirmed-completion semantics. Product implementation must not infer this mode
   from a timeout or hardcode language-specific behavior.
5. ~~Should direct commands tolerate and terminate a server that completed the LSP shutdown
   exchange but did not exit by itself?~~ Resolved in Phase 3: yes, after a successful shutdown
   response and `exit` notification, with a one-second grace period bounded by the user timeout.

Phase 7 found that `home_assistant` and `standardrb` belong to the unsupported-family decision,
leaving 53 capability-only servers and increasing that excluded inventory from 11 to 13.

### Phase 6: implement data-driven best-effort indexing

- Add a validated `build-index-completion` LSP-data field whose default is `confirmed` and whose
  explicit alternative is `best-effort`.
- Mark exactly the 15 servers listed in `docs/SERVERS.md`; do not branch on server or language IDs.
- In best-effort mode, observe the complete bounded wait window and accept only expiry without a
  terminal signal. Preserve failures for malformed messages, transport loss, server-reported index
  errors, and unclean shutdown.
- Remove the 21 obsolete query-exception assertions and keep confirmed behavior for every
  unmarked server and for explicit symbol-query `--wait-for-index` requests.

This improves executable coverage without pretending the protocol confirmed completion. The cost
is that successful `build-index` now has two documented strengths; future server updates must remove
the marker when a reliable terminal signal appears. Immediate success after initialization was
rejected because it observes no work; global timeout acceptance would silently weaken other servers.
`make test` passed, as did all 29 smoke cases selected across the 15 affected servers with `-j10`.

### Phase 7: add specialized capability-only coverage

- Schema v14 adds validated server-level capability coverage: one owner-language case per server
  without pretending every compatible pair satisfies the shared semantic profile.
- A comma-separated `SERVER` selector permits one affected-only `-j10` invocation; selection and
  expected-failure validation recognize generated owner-language cases.
- 53 servers now execute provisioning and capability coverage. Against pinned Mason release
  `2026-09-30-aboard-mob`, 30 initialize successfully; LTeX launchers require explicit generic
  host tools. The remaining 23 servers account for 39 concrete expected failures across
  provisioning and capabilities, documented in the manifest and `docs/GOTCHAS.md`.
- `home_assistant` (Open VSX) and `standardrb` (RubyGems) remain excluded according to the product
  decision, and the complete inventories are updated in `docs/SERVERS.md`.

This widens executable compatibility evidence while retaining precise boundaries: capability-only
does not claim semantic-query support, and expected failure does not claim server support. The
initial tradeoff was 39 broad markers; Phase 8 constrains them by stage and diagnostic. Alternatives
were per-pair capability entries (more duplication)
or retaining all 55 exclusions (no executable evidence).

### Phase 8: constrain expected failures

- Schema v15 lets a marker match its primary failure `stage` and a stable
  `diagnostic-contains` substring. Additional failure stages always remain fatal.
- The coordinator reports a marker mismatch as an unexpected failure and still distinguishes a
  marked case that passed from one that failed differently.
- The 39 Phase 7 markers now constrain their reproduced provisioning or capability diagnostic;
  the older 125 markers retain backward-compatible key-only matching until separately migrated.

This prevents an unrelated registry, protocol, or cleanup failure from satisfying the migrated
markers. The cost is maintenance when upstream wording changes. Removing tolerance would make the
suite unusable with known incompatibilities; a quarantine lane would add CI/reporting complexity.

## Architectural consequences and future risks

- Adding installer backends affects the core download trust boundary and requires archive/path,
  executable-resolution, receipt, cache, and runtime tests for each family.
- Generic initialization options make the data catalog responsible for behavioral launch
  configuration, not just discovery metadata. Schema validation and trust expectations must be
  explicit.
- Mason extra-package constraints make the data catalog partly responsible for repairing upstream
  dependency metadata. PyPI cache signatures prevent stale environments, but each pin requires
  review and removal after an upstream fix. The npm installer also accepts Mason extra packages,
  but its existing cache does not fingerprint dependencies; add equivalent invalidation before
  using data-provided constraints for an npm-backed server.
- A tolerant shutdown policy changes user-visible success semantics. It should distinguish a
  completed LSP exchange from an unknown or interrupted server state.
- Best-effort `build-index` gives command success a weaker, explicitly documented meaning for the
  marked servers; confirmed completion remains the default because LSP lacks a universal signal.
- Expanding scope to linters, formatters, and framework servers changes what "supported LSP
  server" means and may require capability-specific test profiles rather than one general-purpose
  semantic profile.
- The 125 legacy key-only expected-failure markers can still hide unrelated regressions and should
  be migrated from their pinned evidence.
- Server and registry versions drift. Every retained exception or exclusion should identify the
  tested source ID or snapshot when practical.
