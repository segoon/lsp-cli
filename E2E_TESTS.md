# End-to-end test plan

## Goal

Exercise the released `lsp-cli` binary against every supported language, every compatible
supported LSP server, and every top-level subcommand. Keep the suite useful both as a fast pull
request check and as an exhaustive compatibility check.

The tests must validate user-visible behavior: exit status, stdout, stderr, filesystem effects,
server lifecycle, and semantically relevant LSP results. They must not depend on private Rust APIs.

## Working definition of supported

At pinned `lsp-cli-data` revision `013a75f6412917b710aa4683b9c5761c0c679975`, the data tree has:

- 362 filetype configurations;
- 362 LSP configurations;
- 16 detectable filetype IDs (a non-empty `extensions` or `patterns` list);
- 57 distinct LSP configurations associated with those detectable filetypes;
- 141 compatible detectable-filetype/LSP pairs.

The working E2E scope is the 16 detectable IDs:

```text
c cpp cs cuda go gomod gowork java javascript kotlin lua objc objcpp python rust typescript
```

The remaining 346 filetype configurations have no detection rules. They are configuration catalog
entries, but cannot currently drive a project-based E2E test. Separately test that the whole data
tree parses and that catalog commands describe it consistently.

This is a product-policy boundary rather than an implementation fact. Before declaring the suite
complete, the product owner must confirm one of these definitions:

1. **Detectable support (recommended):** exhaustive real-server tests cover the 16 detectable IDs,
   57 relevant servers, and 141 compatible pairs.
2. **Configured support:** all 362 filetype and server configs are considered supported. This first
   requires adding detection rules, test projects, and provisioning for the presently inactive
   catalog entries.

Pros of detectable support: executable now, objective, and automatically derived from the shipped
data. Cons: `lsp-cli languages` currently appears to expose a wider catalog than this definition.

Pros of configured support: the word “supported” matches every shipped YAML entry. Cons: most of
the required projects and provisioning do not exist, and millions of incompatible Cartesian cases
would still need to be excluded.

## Non-goals

- Do not run a Cartesian product of every language, server, command, and option. Only configured
  language/server relationships are meaningful.
- Do not require a server to implement an optional LSP capability.
- Do not put language-specific parsing or source-code knowledge into production `lsp-cli` code.
- Do not make tracked playground files writable test state.
- Do not add a Rust dependency without explicit permission. The existing `tempfile`, `serde`,
  `serde_json`, and `serde_yaml` dependencies are sufficient for the planned harness.

## Test projects

### Existing projects

Reuse the projects under `playground/` for:

| Filetype ID | Directory |
|---|---|
| `c` | `playground/c` |
| `cpp` | `playground/cpp` |
| `cs` | `playground/csharp` |
| `go` | `playground/go` |
| `java` | `playground/java` |
| `javascript` | `playground/js` |
| `lua` | `playground/lua` |
| `python` | `playground/python` |
| `rust` | `playground/rust` |
| `typescript` | `playground/typescript` |

### Projects to add

Add source projects for:

- `playground/cuda`
- `playground/kotlin`
- `playground/objc`
- `playground/objcpp`

Add minimal detection fixtures for `gomod` and `gowork`. These IDs describe Go workspace metadata,
not source languages, so they can cover detection, file listing, server selection, initialization,
and lifecycle, but cannot independently provide meaningful symbol or call-hierarchy assertions.

Every source-language project should be small, valid, and multi-file. Where the language permits,
it should contain:

- one stable workspace symbol;
- functions and methods;
- a declaration separated from its definition;
- references from more than one file;
- a caller and callee chain;
- types and fields;
- a file whose formatting can be made deterministically incorrect;
- a deterministic source mutation that produces one diagnostic.

Prefer equivalent domain concepts and symbol names across projects when natural. Do not force a
language into constructs it does not support merely to make fixtures textually identical.

### Existing playground audit

The ten existing playgrounds were audited against these requirements on 2026-09-05. `Present`
means the committed source provides the semantic shape; `missing` identifies follow-up work;
`unverified` means the required compiler or runtime was not installed for this audit. Under the
strict declaration rule, an interface, trait, protocol, or declaration file is required when the
language can express one without relying on comments or third-party syntax.

| Project | Valid, small, multi-file | Stable workspace symbol | Functions and methods | Separate declaration | Cross-file references | Caller/callee chain | Types and fields | Formatting mutation | Diagnostic mutation |
|---|---|---|---|---|---|---|---|---|---|
| C | Present, but compilation database is not portable | `Order` | Functions present; methods not applicable | Present in `order.h` | Present | Present | Present | Missing recipe; baseline is not formatter-clean | Missing recipe |
| C++ | **Invalid:** undefined `f()` and `g()` prevent linking; compilation database is not portable | `playground::Order` | Present | Present in `order.hpp` | Present | Present | Present | Missing recipe; baseline is not formatter-clean | Missing recipe |
| C# | Unverified; `dotnet` unavailable | `Order` | Present | **Missing:** an interface can provide it | Present | Present | Present | Missing recipe | Missing recipe |
| Go | Unverified; `go` unavailable | `Order` | Present | **Missing:** an interface can provide it | Present | Present | Present | Missing recipe; baseline is visibly not `gofmt`-clean | Missing recipe |
| Java | Unverified; JDK and Maven unavailable | `Order` | Present | **Missing:** an interface can provide it | Present | Present | Present | Missing recipe | Missing recipe |
| JavaScript | Present; exercised with Node.js | `Order` | Present | **Missing:** a declaration file can provide it | Present | Present | Present | Missing recipe | Missing recipe |
| Lua | Unverified; Lua unavailable | **Missing:** only local functions are declared | Functions present; methods missing | Not applicable: Lua has no native declaration construct | Present for `format_timestamp` | Present | Partial: a module table field exists, but no structured domain type | Missing recipe | Missing recipe |
| Python | Present; exercised with Python | `Order` | Present | **Missing:** a protocol or abstract base can provide it | Present | Present | Present | Missing recipe; baseline is visibly not formatter-clean | Missing recipe |
| Rust | **Invalid as a standalone project:** Cargo treats it as an undeclared root-workspace member | `Order` | Present | **Missing:** a trait can provide it | Present | Present | Present | Missing recipe | Missing recipe |
| TypeScript | Unverified; local TypeScript compiler unavailable | `Order` | Present | **Missing:** an interface can provide it | Present | Present | Present | Missing recipe | Missing recipe |

The C sources compile and link, while the C++ sources compile but fail at link time because the
calls added in `main.cpp` have no definitions. JavaScript and Python execute successfully. The Rust
check fails before compilation because the nested package is neither a root-workspace member nor
excluded from that workspace. C and C++ also embed an old absolute checkout path in
`compile_commands.json`; language servers may therefore ignore their intended include paths after
the repository is moved or copied into an isolated E2E sandbox.

No playground currently defines the exact source edit and expected diagnostic needed for a stable
mutation test. Those recipes should live in manifest data rather than language-specific Rust test
code. The next project-phase item will repair and normalize the fixtures; this audit intentionally
does not mix those changes with the inventory.

Tests securely create randomized sandboxes with the `tempfile` crate under the user's
`XDG_RUNTIME_DIR`, `XDG_CACHE_HOME`, or `$HOME/.cache`, in that order, then copy a project there
before formatting it or introducing diagnostics. Real test state must not use the ambient system
`/tmp`. This keeps the repository clean, avoids uncontrolled parent root markers, keeps Unix socket
paths short, and allows safe parallel execution.

Pros of committed playgrounds: humans can reproduce failures with the same projects. Cons: each
language fixture must evolve with its toolchain and server ecosystem.

An alternative is to generate every project during test setup. That reduces committed files, but
makes failures harder to inspect and manual reproduction less convenient; do not use it for the
baseline projects.

## Coverage model

`lsp-cli` currently has 24 canonical top-level subcommands. Factor them by responsibility instead
of multiplying all commands by all language/server pairs.

| Scope | Subcommands | Required coverage |
|---|---|---|
| Global CLI | `commands`, `languages`, `servers`, `completion`, `agent-skill`, `update` | Focused binary-level cases, independent of real servers |
| Detection and filesystem | `detect`, `list-files` | Every detectable filetype ID |
| LSP requests | `server-capabilities`, `diagnostics`, `format`, `grep`, `list-symbols`, `list-functions`, `references`, `callers`, `callees`, `definition`, `declaration`, `build-index` | Every compatible language/server pair, capability-aware |
| Process lifecycle | `run`, `daemon`, `stop`, `stop-all` | Every distinct relevant server where applicable, with grouped lifecycle scenarios |

### Capability-aware expectations

For each compatible pair, first record or inspect the server's initialized capabilities. A command
passes if it either:

- succeeds and returns the expected semantic result; or
- returns the documented, user-facing unsupported-capability error when the server does not
  advertise the required capability.

Formatting, declarations, diagnostics, workspace symbols, and call hierarchy are optional or vary
substantially between servers. Treating every unsupported operation as a suite failure would test
an assumption the LSP specification does not make.

Capability advertisement is not enough by itself: when a server advertises a capability, exercise
the corresponding command and assert its behavior.

### Option coverage

Distribute option variants across the matrix using explicit cases; do not create another full
cross-product. Cover at least:

- automatic selection, `--lang`, and `--lsp`;
- text and `--json` output;
- direct execution, `--detach`, and `--no-detach`;
- `--limit`, `--files-with-matches`, and `--full`;
- `--wait-for-index`;
- `format`, `format --check`, and `format --stdout`;
- successful operations, unsupported capabilities, missing executables, server crashes, malformed
  replies, and timeouts;
- `--download` once per supported installation mechanism, rather than redundantly for every query.

JSON assertions should deserialize and compare stable semantic fields. Text assertions should
avoid full snapshots when server versions can legitimately change ordering, signatures, or detail.

## Harness design

Use a normal Cargo integration-test crate that invokes the built binary through
`CARGO_BIN_EXE_lsp-cli`.

Proposed layout:

```text
tests/
  e2e.rs
  e2e/
    harness.rs
    manifest.rs
    catalog.rs
    detection.rs
    queries.rs
    lifecycle.rs
    update.rs
    cases.yaml
```

Keep every Rust file under 600 lines. Move repeated process setup and assertions into helpers as
soon as a second test needs them.

`harness.rs` should provide methods for actions on an E2E context, for example:

- create an isolated home, configuration root, runtime root, and workspace copy;
- construct an `lsp-cli` process with deterministic environment variables;
- run a command with a deadline and capture stdout/stderr/status;
- parse JSON output;
- introduce a formatting or diagnostic mutation;
- find and terminate remaining child processes;
- stop daemons and report their runtime state after failure.

Each test process should set at least:

- `HOME` to an isolated temporary home;
- `XDG_CONFIG_HOME` to an isolated configuration directory;
- `XDG_RUNTIME_DIR` to an isolated daemon directory;
- `LSP_DATA` to the pinned repository submodule;
- `PATH` to the explicitly provisioned toolchain/server environment.

Do not rely on a developer's user configuration, downloaded server cache, daemon sockets, current
shell, or ambient server versions.

The manifest should include stable case data, provisioning metadata, expected capabilities, and
documented exclusions. A validation test should fail when:

- a detectable filetype lacks a project;
- a compatible pair lacks a manifest entry;
- a manifest entry names a missing data config;
- an exclusion lacks a reason;
- two cases select the same user-visible server ambiguously;
- a new top-level subcommand has no assigned coverage class.

The version 2 manifest assigns every canonical command to a coverage strategy and keeps
`coverage: partial`, which validates every declared language/server entry
against the pinned data without requiring unfinished matrix entries. Phase 4 adds the remaining
entries and switches it to `coverage: complete`; complete mode enforces every detectable language
and compatible pair.

### Extending the manifest

To cover an existing filetype, add its small project under `playground/`, declare it once under
`languages`, then add a `pairs` entry for each compatible server. To introduce a genuinely new
filetype or server, first add its YAML config and commit it in the `data` submodule, then update the
submodule revision and the E2E manifest in this repository.

Pair entries use the LSP YAML filename stem as their stable config ID. The test runner loads the
configured user-visible server name for `--lsp`; do not duplicate it in the manifest. Each optional
`smoke` block declares a generic provisioning method, query kind, semantic expectations, runtime
host programs, and deadlines. Language-specific prerequisites and expected symbols belong in YAML,
not in the Rust runner. The first provisioning method is `download`; add other mechanisms as typed
methods when needed instead of branching on server names.

In `coverage: complete` mode, manifest validation makes a new detectable filetype or compatible
filetype/server relationship fail until its project and pair are declared. Partial mode intentionally
allows the matrix to grow incrementally.

## Special command strategies

### `run`

`run` replaces the current process with the language server on Unix. The foundation smoke uses a
deterministic server marker to prove replacement. Real-server coverage should additionally use
piped stdio for a minimal `initialize` / `initialized` / `shutdown` / `exit` exchange. Test
selection and exec errors separately.

### `daemon`, `stop`, and `stop-all`

For every applicable server, run a grouped lifecycle scenario:

1. start a daemon in an isolated runtime directory;
2. issue at least two queries with `--detach` and verify reuse;
3. stop the exact daemon;
4. verify that a later query starts or connects according to documented behavior;
5. start multiple isolated daemons and verify `stop-all` removes all of them.

On failure, print socket paths, process state, selected command line, workspace root, and bounded
server stderr. Cleanup must run even after an assertion failure.

### `update`

The production default uses the lsp-cli-data GitHub release endpoint. The narrowly scoped
`LSP_CLI_DATA_RELEASE_API_URL` override redirects only release metadata lookup, allowing the binary
E2E test to serve metadata and a valid archive locally. This keeps the success path deterministic
without changing normal update behavior.

### Diagnostics and formatting

Start from a valid temporary workspace. Apply one language-specific mutation recorded in the
manifest, run the command, assert the expected file/range/message class, and discard the temporary
copy. Do not commit permanently broken source files that could interfere with unrelated queries.

### Indexing

`build-index` should be tested against every server that has a usable background-work completion
signal. For other servers, assert the intended bounded timeout or no-op policy. Do not infer
completion from a fixed sleep.

## Real-server provisioning

Pin every server and required toolchain version. The manifest should distinguish:

- directly installed executables;
- npm, PyPI, Cargo, Go, or other package-manager installations;
- archive-based installations;
- servers requiring a language SDK or compiler;
- servers not installable through the current downloader.

Do not silently skip a required pair because its executable is absent. A CI lane either provisions
the server or reports the pair as an explicit, reviewed exclusion.

Adding and pinning these external test tools requires product-owner approval under the repository's
dependency policy. They need not become Rust package dependencies, but they are still operational
dependencies with maintenance, security, licensing, storage, and network consequences.

Pros of pinned versions: reproducible failures and controlled upgrades. Cons: compatibility with
new upstream releases is detected only when pins are deliberately refreshed.

An unpinned “latest” lane can complement the pinned suite on a schedule. Its advantage is early
warning of upstream breakage; its disadvantage is nondeterminism, so it must not be the only merge
gate.

## CI plan

### Pull requests

Run:

- all existing unit tests and checks through `make test`;
- global and detection E2E tests;
- one preferred, pinned server per source language;
- every relevant subcommand across that smoke matrix;
- manifest/data consistency checks.

### Nightly exhaustive matrix

Run all 141 compatible pairs, sharded by language and server installation family. Use fail-fast
disabled so one broken server does not hide the rest of the compatibility report.

Cache downloaded toolchains and server packages using keys that include the pinned version. Do not
share homes, daemon runtime directories, or mutable workspaces between parallel jobs.

### Scheduled latest-version compatibility

Optionally run supported servers at current upstream versions. Report failures separately from the
pinned merge gate until a human confirms whether the server or `lsp-cli` needs adaptation.

### Manual workflow

Allow selection of one language, one server, or one installation family. This is needed to debug a
nightly failure without rerunning the complete matrix.

Pros of split CI: fast merge feedback plus exhaustive coverage. Cons: a regression affecting a
non-preferred server may be found the following night rather than on the originating pull request.

Running all 141 pairs on every pull request gives earlier detection, but has much higher latency,
cost, rate-limit exposure, and upstream-flake risk. It is not the recommended default.

## Failure policy

Classify failures as:

1. `lsp-cli` regression;
2. playground/manifest drift;
3. provisioning or network failure;
4. upstream server behavior change;
5. known server limitation;
6. unsupported LSP capability with the expected user-facing response.

Only category 6 is an immediate passing outcome. Known limitations must be explicit manifest
entries and, when they concern protocol or server behavior, documented in `GOTCHAS.md`. Do not add
unbounded retries. A retry may cover an identified transient installation/network step, but must
not conceal query or protocol failures.

If a hard-to-debug defect is fixed, add a focused regression test in addition to the broad matrix.
Also consider whether a type invariant, runtime check, clearer trace, or state-dump helper can make
that class of defect easier to diagnose.

## Execution phases

### Phase 0: approve boundaries

- [x] Confirm detectable support versus configured support: use the 16 detectable IDs.
- [x] Confirm that unsupported optional capabilities count as a passing, asserted outcome.
- [x] Approve pinned external server/toolchain provisioning.
- [x] Approve a narrow HTTP endpoint seam for deterministic `update` E2E coverage.
- [x] Confirm PR smoke plus nightly exhaustive CI cadence.

### Phase 1: foundation

- [x] Add `tests/e2e.rs` and compact harness modules.
- [x] Add the initial manifest schema and validation.
- [x] Isolate all environment and runtime state.
- [x] Implement deadlines, cleanup guards, JSON helpers, and useful failure diagnostics.
- [x] Prove the harness with Rust/rust-analyzer.
- [x] Cover all 24 subcommand paths with either a real server or a deterministic local fixture.

### Phase 2: projects

- [x] Audit the ten existing playgrounds against the common semantic requirements.
- [x] Remove duplicated setup patterns within each class of fixture.
- [ ] Add CUDA, Kotlin, Objective-C, and Objective-C++ projects.
- [ ] Add `gomod` and `gowork` detection fixtures.
- [ ] Update `playground/README.md` with manual reproduction commands.
- [ ] Run each relevant command manually against every new project.

### Phase 3: preferred-server smoke matrix

- [ ] Select and pin one preferred server for each source language.
- [ ] Add provisioning scripts without new Rust dependencies.
- [ ] Implement capability-aware query assertions.
- [ ] Implement direct/detached lifecycle scenarios.
- [ ] Add the pull-request E2E job.

### Phase 4: exhaustive compatibility

- [ ] Populate manifest entries for all 141 compatible pairs.
- [ ] Provision every non-excluded server and required SDK.
- [ ] Record reviewed exceptions and platform constraints.
- [ ] Add sharded nightly and manual workflows.
- [ ] Verify failures retain server version, command line, capabilities, stderr summary, and cleanup
  state.

### Phase 5: hardening

- [ ] Run `make test`.
- [ ] Run the full pinned E2E matrix from a clean environment.
- [ ] Check every new or edited test file for boilerplate and duplication.
- [ ] Check every source file remains below 600 lines.
- [ ] Add regression tests for every bug uncovered during rollout.
- [ ] Add LSP/server-specific discoveries to `GOTCHAS.md`.
- [ ] Document how to refresh server pins and triage nightly failures.

## Definition of done

The work is complete when:

- every accepted supported language has a committed project or justified metadata-only fixture;
- every compatible supported language/server pair has an executable manifest entry;
- every top-level subcommand has binary-level E2E coverage in its appropriate scope;
- advertised capabilities are exercised and unsupported capabilities have asserted user-facing
  behavior;
- direct, detached, stop, and stop-all lifecycle paths are covered;
- formatting and diagnostics cannot dirty tracked files;
- required servers and toolchains are pinned and reproducibly provisioned;
- PR smoke, nightly exhaustive, and manual targeted workflows are documented and passing;
- `make test` passes;
- known protocol/server deviations are recorded in `GOTCHAS.md`;
- no required case is silently skipped.

## Architectural consequences and limitations

- The data catalog becomes an enforceable compatibility contract: adding a detectable filetype or
  compatible LSP config requires E2E ownership.
- Real-server E2E tests are inherently slower and less hermetic than protocol tests. Unit tests and
  fake-server integration tests remain necessary for precise edge cases.
- Capability-aware results mean “all commands tested” does not mean “all commands succeed on every
  server.” It means every applicable success path and every inapplicable user-facing response is
  verified.
- Third-party server pinning creates a recurring upgrade and security-review obligation.
- Some servers require proprietary, platform-specific, or unusually heavy SDKs. Their treatment
  must be an explicit product decision rather than an automatic skip.
- The current difference between the 362 configured filetypes and 16 detectable filetypes may need
  a future terminology or behavior change in `languages`; this plan exposes but does not decide
  that product question.
