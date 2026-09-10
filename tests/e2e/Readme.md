# End-to-end tests

## Goal

Exercise the released `lsp-cli` binary against every supported language, every compatible
supported LSP server, and every top-level subcommand. Keep the suite useful both as a fast pull
request check and as an exhaustive compatibility check.

The tests validate user-visible behavior: exit status, stdout, stderr, filesystem effects, server
lifecycle, and semantically relevant LSP results. They do not depend on private Rust APIs.

## Scope

"Supported" means **detectable support**: real-server tests cover every filetype ID that has a
detection rule (a non-empty `extensions` or `patterns` list), every LSP config compatible with
those filetypes, and every resulting compatible language/server pair. A filetype configuration
with no detection rule is a configuration catalog entry only — it cannot drive a project-based E2E
test, so those entries are covered separately by parsing/catalog consistency checks, not real
queries.

The current inventory (detectable filetypes, relevant servers, compatible pairs) is derived
directly from the pinned `lsp-cli-data` submodule revision and drifts as that data changes; treat
`tests/e2e/cases/` (one YAML file per language ID) and `tests/e2e/cases/suite.yaml`
(`schema-version`, `coverage: complete`) as the source of truth for current counts rather than any
number written here.

Open product question (not yet decided): the full data catalog has more configured filetypes than
detectable ones. Whether "supported" should eventually mean every configured filetype (which would
require adding detection rules, test projects, and provisioning for the currently inactive catalog
entries) is a product-policy call, not an implementation fact.

## Non-goals

- Do not run a Cartesian product of every language, server, command, and option. Only configured
  language/server relationships are meaningful.
- Do not require a server to implement an optional LSP capability.
- Do not put language-specific parsing or source-code knowledge into production `lsp-cli` code.
- Do not make tracked playground files writable test state.
- Do not add a Rust dependency without explicit permission. The existing `tempfile`, `serde`,
  `serde_json`, and `serde_yaml` dependencies are sufficient for the harness.

## Local dev environment

The real-server E2E targets (`test-real-server-e2e`, `test-real-server-smoke-e2e`,
`test-server-provisioning-e2e`) need `go`, `java`, `node`/`npm`, and `dotnet` on `PATH`, matching
what CI installs in `.github/workflows/ci.yml` and `e2e.yml`. Run `make download-dev-env` to fetch
project-local copies into `.env/` (not a system-wide install), then `source activate.sh` from the
repo root to put them on `PATH` for the current shell.

## Test projects (playgrounds)

Real-server cases use small, committed multi-file projects under `playground/`; see
`playground/README.md` for the per-project layout and manual reproduction commands. Where the
language permits, every source-language project should contain:

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

Filetype IDs that describe workspace/module metadata rather than a source language (e.g.
`gomod`/`gowork`) use minimal detection-only fixtures: they cover detection, file listing, server
selection, initialization, and lifecycle, but cannot independently provide meaningful symbol or
call-hierarchy assertions.

Tests securely create randomized sandboxes with the `tempfile` crate under the user's
`XDG_RUNTIME_DIR`, `XDG_CACHE_HOME`, or `$HOME/.cache`, in that order, then copy a project there
before formatting it or introducing diagnostics. Real test state must not use the ambient system
`/tmp`. This keeps the repository clean, avoids uncontrolled parent root markers, keeps Unix socket
paths short, and allows safe parallel execution.

Committed playgrounds (vs. generating every project during test setup) trade "each language
fixture must evolve with its toolchain and server ecosystem" for "humans can reproduce failures
with the same projects" — keep using committed projects for the baseline fixtures.

## Coverage model

`lsp-cli`'s canonical top-level subcommands are factored by responsibility instead of multiplying
all commands by all language/server pairs:

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

JSON assertions deserialize and compare stable semantic fields. Text assertions avoid full
snapshots when server versions can legitimately change ordering, signatures, or detail.

## Harness & manifest layout

A normal Cargo integration-test crate invokes the built binary through `CARGO_BIN_EXE_lsp-cli`:

```text
tests/
  e2e.rs
  e2e/
    harness.rs
    manifest.rs / manifest/
    catalog.rs
    queries.rs
    lifecycle.rs
    update.rs
    cases/
      suite.yaml
      <language>.yaml
```

Keep every Rust file under 600 lines; move repeated process setup and assertions into helpers as
soon as a second test needs them.

`harness.rs` provides methods for actions on an E2E context: creating an isolated home,
configuration root, runtime root, and workspace copy; constructing an `lsp-cli` process with
deterministic environment variables; running a command with a deadline and capturing
stdout/stderr/status; parsing JSON output; introducing a formatting or diagnostic mutation; finding
and terminating remaining child processes; stopping daemons and reporting their runtime state after
failure.

Each test process sets at least:

- `HOME` to an isolated temporary home;
- `XDG_CONFIG_HOME` to an isolated configuration directory;
- `XDG_RUNTIME_DIR` to an isolated daemon directory;
- `LSP_DATA` to the pinned repository submodule;
- `PATH` to the explicitly provisioned toolchain/server environment.

Do not rely on a developer's user configuration, downloaded server cache, daemon sockets, current
shell, or ambient server versions.

`tests/e2e/cases/suite.yaml` owns global command coverage and assigns every canonical command a
coverage strategy; each `tests/e2e/cases/<language>.yaml` owns one project and its configured
server behavior. Case YAML contains only E2E-specific behavior — it does not duplicate each
server's `filetypes` list, which is derived from `data/lsp-cli.yaml`. Pair entries are sparse E2E
behavior overlays keyed by the LSP YAML filename stem as their stable config ID; bare compatibility
entries are rejected because compatibility belongs to `data`.

A validation test fails when:

- a detectable filetype lacks a project;
- a configured E2E case names an incompatible or missing data config;
- an exclusion lacks a reason;
- two cases select the same user-visible server ambiguously;
- a new top-level subcommand has no assigned coverage class.

Every relevant server has exactly one preferred pair per source language, tagged with a `smoke`
disposition (a generic query suite, or an exclusion with a mandatory reviewed reason) and exactly
one lifecycle-owner pair, tagged with a `lifecycle` disposition (grouped daemon scenarios, or a
reviewed exclusion). Direct `run` may be excluded independently when only detached operation is
reliable. This keeps the chosen project stable when a shared server gains another filetype, without
repeating process tests for every compatible pair. Source-language query profiles declare shared
semantic terms, expected symbols, and format paths; pair entries keep only deadlines and narrowly
scoped known-result exceptions (see "Real-server exceptions" below). Language-specific
prerequisites and expectations belong in YAML, not in the Rust runner.

### Extending the manifest

To cover an existing detectable filetype, add its small project under `playground/` and one case
file named after the filetype ID. Compatible servers are discovered from `data/lsp/*.yaml`; add a
`pairs` entry only when the E2E suite has executable behavior or a reviewed exclusion for that
pair. To introduce a genuinely new filetype or server, first add its YAML config and commit it in
the `data` submodule, then update the submodule revision, then add a project/case file here and any
pair-specific E2E behavior.

The query runner obtains raw initialized capabilities through `server-capabilities --json`, then
executes every LSP query command. Advertised capabilities require a successful semantic response;
missing capabilities require the command's user-facing unsupported error.

## Real-server exceptions

In `tests/e2e/manifest/query_case.rs`, a `smoke` pair can be `status: queries`, and each query case
carries an optional `exceptions` list. Each entry names a `command` (one of the real-server query
kinds — `grep`, `references`, `callers`, `callees`, `build-index`, `format`, etc.), an `outcome`
(`failure` or `empty-matches`), an optional expected stderr `message`, and a mandatory `reason`.

At runtime (`tests/e2e/real_servers.rs`), if a query has a matching exception, the harness skips
the normal "must succeed with real matches" assertion and instead asserts the *documented* deviant
behavior:

- `failure`: the command must exit non-zero and stderr must contain `message`.
- `empty-matches`: the command must succeed but return an empty `matches` array.

`exceptions` is not error-tolerance or flakiness suppression — it's a positive assertion of each
server's known, reproducible protocol quirk, with the `reason` pinned in the YAML so the deviation
is self-documenting and any regression still fails loudly.

### Known root causes

- **No background-indexing-completion signal** (`build-index` → `failure`, message
  "background-work progress"). Several servers advertise `$/progress`/work-done tokens but never
  send a terminal "index build finished" notification the CLI can wait on. This is the single most
  common exception across the suite.
- **Workspace-symbol search (`grep`) racing indexing.** A server with no synchronous "ready" signal
  can return empty `matches` for `workspace/symbol` issued immediately after startup.
  `run_workspace_symbol_query` (`src/commands/symbol_query.rs`) primes the server by opening a
  workspace document and retries with a short poll when the first `workspace/symbol` call is empty
  or errors; servers that still race past that retry keep an `empty-matches` exception.
- **Call-hierarchy has no edges for the fixture.** A server reports empty `callees`/`callers` when
  the fixture doesn't happen to exercise a real call edge for the queried symbol, or (for a couple
  of servers) as a genuine analysis limitation independent of the fixture.
- **Server-specific formatting/output bugs**, e.g. a `format` edit whose range falls outside the
  requested file — tolerated as a `failure` exception with a matched message.

### Servers excluded entirely (`status: excluded`)

- `denols` (Deno LSP) rejects the standard shutdown request because it requires non-null params.
- `roslyn_ls` (cs) and `lua_ls` (lua) have lifecycle-level incompatibilities: no smoke queries at
  all, or no clean exit after direct shutdown.
- Several Python servers fail to launch/initialize correctly in the isolated harness: `pylsp`
  (Mason launcher can't import the module), `pyre` (same), `pyrefly` (initializes but returns no
  workspace/document symbols).

Real LSP servers deviate from the LSP spec's strict guarantees in ways that are reproducible but
server-specific. Rather than weakening assertions globally, the suite encodes each deviation
explicitly per server/command so real regressions still fail loudly, while known quirks are pinned
and self-documented via `reason`.

## Provisioning

Do not install LSP servers separately. Every real-server case passes `--download`, letting the
production Mason integration select the current registry package, install it inside the case's
isolated home, and return the resolved executable. This applies uniformly to direct archives and
npm, PyPI, Cargo, Go, NuGet, GitHub, or generic package sources supported by the downloader.

Language SDKs and package-manager runtimes remain explicit host prerequisites, with their resolver
commands kept in the manifest so a missing prerequisite produces a case-specific error rather than
a silent skip. A CI lane either provisions the server or reports the pair as an explicit, reviewed
exclusion — a required pair is never silently skipped because its executable is absent.

Downloading these external test tools requires product-owner approval under the repository's
dependency policy. Latest-Mason server downloads and official free SDK/runtime provisioning are
approved; they do not become Rust package dependencies, but remain operational dependencies with
maintenance, security, licensing, storage, and network consequences. Proprietary and
unsupported-platform prerequisites are excluded.

Always using Mason latest detects upstream compatibility changes immediately and avoids maintaining
a second installation path. The tradeoff is a nondeterministic merge gate: a registry or server
release can break an unchanged commit, and reproducing the failure depends on the recorded source
ID remaining available upstream.

Run the complete downloadable-server inventory manually with:

```sh
make test-server-provisioning-e2e
```

Set `E2E_SERVER=<config-id>` to diagnose one downloadable server. The test copies the server's
owner project into an isolated context, stages its declared host programs, invokes `detect` with
`--download`, and verifies that exactly one selected command resolves inside the isolated home. It
does not initialize the server or substitute for the later language/server behavior matrix.

## Running and reproducing tests

From the repository root:

```sh
# smoke: one preferred server per source language, all relevant subcommands
E2E_CASE=python/pyright make test-real-server-smoke-e2e

# combined query + lifecycle target for one pair
E2E_CASE=java/jdtls make test-real-server-e2e

# provisioning only, for one downloadable server
E2E_SERVER=pyright make test-server-provisioning-e2e
```

`E2E_CASE=<language>/<server-id>` (or `E2E_CASES=` with a comma-separated list, used by sharded CI)
selects one configured executable or explicitly excluded case without changing the all-cases
default. Selecting a compatible pair with no E2E behavior fails with a clear error instead of
silently running no tests.

These commands download external tools and require the host programs declared by the manifest, and
they use the current Mason registry — compare the resulting source ID with an earlier run before
concluding that local behavior has changed.

The manual **End-to-end compatibility** GitHub Actions workflow can select `language`, `server`, or
`installation-family` (the value is respectively a case language ID, an LSP config ID, or one of
`cargo generic github golang npm nuget pypi`):

```sh
gh workflow run e2e.yml -f selector=server -f value=pyright
```

Use `selector=all` with an empty value for the complete matrix. The generated workflow summary
shows all selected pairs, including reviewed exclusions, before runnable pairs are sharded.

## CI plan

**Pull requests** run: all existing unit tests and checks via `make test`; global and detection E2E
tests; one preferred current-Mason server per source language; every relevant subcommand across
that smoke matrix; manifest/data consistency checks.

**Nightly** runs all compatible pairs, sharded by language and server installation family, with
fail-fast disabled so one broken server doesn't hide the rest of the compatibility report. The
planner resolves installation families from the current Mason registry rather than copying that
registry metadata into `cases/`. Suite-level smoke, lifecycle, and provisioning deadlines provide
common defaults; cases only declare intentional overrides.

Jobs never share homes, daemon runtime directories, or mutable workspaces. CI may cache immutable
download transport data, but each case retains isolated runtime state and never substitutes a
separately installed server for `--download`. Every real-server case tears down its isolated home
and temporary roots (Mason packages, Go module/build caches, other server download state) before
the next case starts; only immutable Rust build artifacts are shared by CI.

Split CI (fast PR smoke + exhaustive nightly) trades "a regression affecting a non-preferred server
may surface the following night rather than on the originating PR" for much lower latency, cost,
rate-limit exposure, and upstream-flake risk on every PR — this is the deliberate default over
running everything on every PR.

## Triage a CI failure

Pairs use `<language>/<server-config-id>`, e.g. `python/pyright`. The server component is the
filename stem under `data/lsp/`, not necessarily the executable or display name.

1. Open the workflow summary and find the pair's executable or excluded classification.
2. Open the failed `<language>/<installation-family>` matrix job.
3. Read the `E2E failed case IDs` footer, which lists the specific failed pairs collected from the
   diagnostics above it. If the footer says the IDs are unavailable, failure occurred before a case
   emitted its labelled diagnostic — start with the planner, build, or test-runner error
   immediately above the footer.

### Identify the upstream version

A failed case retains this block before deleting its isolated home:

```text
server package source IDs:
pkg:npm/pyright@1.1.409
server command line:
...
server capabilities:
...
server stderr summary:
...
cleanup state:
...
```

Treat each complete `pkg:<installation-family>/<package>@<version>` source ID as the authoritative
package identity — preserve the whole value, since versions and package names can contain
prefixes, scopes, or backend-specific suffixes. The executable name and `initialize` response
version may describe a product differently and are supporting evidence, not replacements for the
source ID.

`<unavailable: no completed server installation>` normally means provisioning failed before a
receipt was written; inspect the preceding download/install error rather than inferring a version
from an older run. Outside the isolated suite, successful downloads store JSON receipts under
`~/.local/share/lsp-cli/receipts/` (same `source_id` field meaning). E2E case homes and their
receipts are intentionally removed after every case, including failures, so use the retained
diagnostic rather than a path printed earlier in the log.

Successful E2E cases print no retained failure context. The suite follows Mason latest, so a later
rerun may resolve a different source ID — compare IDs from available failing runs and record the ID
in an issue when exact upstream identity matters; the suite does not promise the registry will
retain an older version for reproduction.

### Classify the failure

Check evidence in this order:

1. **Planner or manifest:** an unknown selector, missing registry package, or validation failure
   happened before a server case ran.
2. **Provisioning or network:** no completed receipt, package-manager output, HTTP failure, or an
   absent host program points to installation rather than LSP behavior — use
   `make test-server-provisioning-e2e`.
3. **Startup or shutdown:** use the retained command line and server stderr. SDK incompatibility,
   launcher failure, crash, and failure to exit are distinct from query-result drift.
4. **Protocol or capability:** compare the retained capabilities with the command exercised by the
   case. An unadvertised optional capability passes only when lsp-cli returns its expected
   user-facing unsupported error.
5. **Semantic result:** compare stable names and the pair's reviewed exceptions (see "Real-server
   exceptions" above). Do not weaken an expectation until the same source ID reproduces the
   behavior or an upstream change is confirmed.
6. **Cleanup:** inspect sandbox and runtime roots independently of the primary failure — a
   successful query with a retained root is still a cleanup regression.

## Failure policy

Classify failures as:

1. `lsp-cli` regression;
2. playground/manifest drift;
3. provisioning or network failure;
4. upstream server behavior change;
5. known server limitation;
6. unsupported LSP capability with the expected user-facing response.

Only category 6 is an immediate passing outcome. Known limitations must be explicit manifest
entries and, when they concern protocol or server behavior, documented in `docs/GOTCHAS.md`. Do not add
unbounded retries — a retry may cover an identified transient installation/network step, but must
not conceal query or protocol failures.

If a hard-to-debug defect is fixed, add a focused regression test in addition to the broad matrix,
and consider whether a type invariant, runtime check, clearer trace, or state-dump helper can make
that class of defect easier to diagnose next time.

## Definition of done (for a new language/server)

- The language has a committed project or a justified metadata-only fixture.
- Every compatible supported language/server pair has an executable manifest entry.
- Every top-level subcommand exercised has binary-level E2E coverage in its appropriate scope.
- Advertised capabilities are exercised; unsupported capabilities have asserted user-facing
  behavior.
- Direct, detached, stop, and stop-all lifecycle paths are covered where applicable.
- Formatting and diagnostics tests cannot dirty tracked files.
- Required servers are provisioned through `--download`, and resolved source IDs are retained on
  failure.
- Known protocol/server deviations are recorded in `docs/GOTCHAS.md`.
- No required case is silently skipped.
