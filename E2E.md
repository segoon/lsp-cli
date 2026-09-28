# End-to-end compatibility remediation plan

## Status and scope

This document records the investigation of GitHub Actions run
[`36113662065`](https://github.com/segoon/lsp-cli/actions/runs/36113662065) and proposes a
step-by-step remediation plan. The run was the scheduled **End-to-end compatibility** workflow at
commit `2840940c8e5186194fadf5322833862b40b7b416` on 2026-09-25.

The investigation below is retained as the historical baseline. Implementation completed on
2026-09-28: Phases 0-7 are implemented, and Phase 8 local verification passes. The final hosted
full-matrix run must be dispatched after the commit is pushed so GitHub can test that exact SHA.

Implemented outcomes:

- structured per-case failure reports retain source IDs and registry snapshot identity;
- intrinsic runtimes, Cargo/rustup state, Go source subpaths, dependency download caching, and
  rate-limit retries are handled explicitly;
- PyPI packages install into versioned per-package virtual environments rather than broken prefix
  layouts;
- the current PyPI family and Rust Analyzer regression pass locally, while remaining incompatible
  pairs have reviewed reasons and relevant server gotchas;
- CI uses the green-health-gate model, with a fast pull-request matrix and exhaustive nightly or
  manually dispatched compatibility matrix;
- `make test` passes with rustls updated to the patched 0.23.45 release.

The immediate objective is to make the nightly result actionable: a red job should identify a new
regression or a genuinely unsupported pair, rather than repeat a known harness or installer defect
across dozens of shards.

## Investigation summary

The workflow infrastructure itself completed successfully:

- compatibility-matrix discovery succeeded;
- the Mason registry snapshot was created and downloaded by the test jobs;
- Rust, Node, Go, Java, and .NET setup steps ran when selected by the matrix;
- failures occurred in `Run real-server smoke cases`, after the matrix had been built.

The matrix result was:

| Installation family | Passed | Failed |
| --- | ---: | ---: |
| Cargo | 0 | 22 |
| Generic | 2 | 6 |
| GitHub | 48 | 78 |
| Go | 4 | 6 |
| npm | 4 | 49 |
| NuGet | 2 | 0 |
| PyPI | 1 | 22 |
| **Total** | **61** | **183** |

The job-name/outcome set was unchanged from the preceding nightly. The broad failure pattern is
therefore reproducible and is not explained by one transient GitHub or package-host outage.

The current `master` differs from the failing commit only by unrelated Clap dependency updates, so
there is no later fix already present in the repository.

## Findings

### 1. The isolated server `PATH` hides required runtimes

The E2E harness clears the inherited environment and runs `lsp-cli` with a `PATH` containing only
the case's isolated bin directory. The original host `PATH` is exposed separately through
`LSP_CLI_INSTALL_PATH`, allowing installers to be located without allowing server discovery to use
ambient LSP executables.

This separation is architecturally useful, but installed servers often remain scripts. For example,
an npm launcher commonly uses `#!/usr/bin/env node`. Unless `node` is explicitly staged into the
isolated bin directory, the installed launcher resolves successfully but cannot start.

Representative error:

```text
failed to initialize yaml-language-server:
/usr/bin/env: 'node': No such file or directory
```

The matrix planner provisions Node for every npm installation shard, but provisioning Node on the
runner does not stage it into the case's server `PATH`. The four successful npm shards declare Node
as a host program; most of the 49 failed npm shards do not. The same issue affects some GitHub
packages that launch through `bash`, Node, Elixir, or another secondary runtime.

Consequences:

- provisioning tests can pass after finding an executable while initialization still fails;
- installation-family setup and case-level host-program declarations currently express overlapping
  but different requirements;
- adding the complete host `PATH` back would make the tests pass for the wrong reason and could
  silently select an ambient server instead of the downloaded one.

### 2. Cargo is found, but rustup state is removed

Cargo installation uses `LSP_CLI_INSTALL_PATH`, so the rustup-provided `cargo` shim can be found.
However, the E2E harness also replaces `HOME` and removes ambient variables. The shim can no longer
find the configured toolchain.

At least 21 of the 22 Cargo-family failures have the same form:

```text
cannot install <package> because cargo failed:
help: run 'rustup default stable' to download the latest stable release of Rust and set it as your
default toolchain.
```

The remaining Cairo shard is independent: its selected Mason package does not expose the configured
`scarb` executable.

This is primarily a harness-environment defect, not evidence that those Cargo packages are broken.
Passing only `PATH` to installer processes is insufficient for tool managers whose configuration is
stored outside the isolated `HOME`.

### 3. Go package fragments are appended to the version

The current source-ID parser separates `?qualifiers`, but it does not parse a `#subpath` fragment.
For Mason source IDs such as:

```text
pkg:golang/cuelang.org/go@v0.17.1#cmd/cue
```

the installer constructs:

```text
go install cuelang.org/go@v0.17.1#cmd/cue
```

Go treats `v0.17.1#cmd/cue` as the version and rejects it. Six Go-family shards fail this way;
packages without a fragment, including `gopls`, succeed.

The intended install target is expected to be the module plus subpath, followed by the version:

```text
go install cuelang.org/go/cmd/cue@v0.17.1
```

This interpretation must be verified against current Mason registry semantics before implementation.
The parser should retain the fragment as structured source metadata instead of teaching the Go
installer to manipulate an opaque source string.

### 4. PyPI `--prefix` launchers cannot import installed modules

PyPI packages are installed with:

```text
python3 -m pip install --prefix <package-directory> ...
```

The generated scripts are executable, but the system Python interpreter does not automatically add
that prefix's `site-packages` directory to `sys.path`. Twenty-two of 23 PyPI shards fail with errors
such as:

```text
ModuleNotFoundError: No module named 'cmake_language_server'
```

This problem is already recorded in `docs/GOTCHAS.md`. It also demonstrates that a provisioning
test which checks only executable resolution is weaker than an initialization test.

### 5. A successful download does not imply a runnable server configuration

The GitHub and generic families contain several independent classes of incompatibility:

- literal installation placeholders, such as `/path/to/...`, remain in some command lines;
- some packages do not expose the executable named by the data configuration;
- some servers require user-specific setup, for example an Arduino CLI configuration;
- some packages declare no artifact for the current platform;
- script-based releases need undeclared host programs such as `bash` or Elixir;
- some downloads or upstream endpoints fail;
- some servers initialize and answer the request but close during `shutdown` or never exit before
  the deadline.

Representative cases observed in the run include Arduino Language Server, Elixir LS, Groovy LS,
YQL, PHPActor, `r-languageserver`, Vala Language Server, `neocmakelsp`, Helm LS, Bazelrc LSP, and
Tofu LS. These examples are not an exhaustive classification of all 84 failed GitHub/generic
shards; each remaining pair needs individual triage after the systemic failures are removed.

### 6. Shutdown behavior is part of user-visible success

Several servers return useful capabilities and then fail the command because shutdown is unclean:

- the server closes while the client is waiting for a `shutdown` response;
- the server answers `shutdown` but does not exit before the deadline.

Ignoring these errors would make the capability query look successful while leaving a process or
reporting a failure later. Any relaxation must be based on the LSP specification and explicit
process-cleanup guarantees, not a list of server names hidden in generic client code.

These newly observed server-specific behaviors should be added to `docs/GOTCHAS.md` when their
individual dispositions are implemented. They were not added during the investigation because the
investigation was explicitly read-only.

### 7. The nightly workflow's desired meaning is not yet explicit

The documentation calls the workflow an exhaustive compatibility report, but GitHub represents the
183 known failures as a failed workflow. There are two reasonable product meanings:

1. a health gate which is expected to be green after known incompatibilities are excluded; or
2. an audit which intentionally attempts unsupported pairs and reports failures without treating
   them as workflow regressions.

This is an architectural/product decision and must be made by the project owner. Implementation
should not silently choose one by adding `continue-on-error` or broad exclusions.

## Recommended design principles

The remediation should preserve these properties:

1. **Downloaded-server identity remains isolated.** Server lookup must not fall through to the full
   host `PATH`.
2. **Installer tools and server runtimes are different concepts.** Cargo may be needed only during
   installation, while Node may be needed both during npm installation and every time the installed
   server starts.
3. **Runtime requirements are data, not language assumptions.** Do not infer comment syntax,
   character sets, or other source-language details.
4. **Package-source parsing is structured and extensible.** Preserve qualifiers and subpaths for
   future installation backends rather than embedding one-off string rewrites.
5. **Known incompatibilities are explicit.** Each exclusion needs a user-readable reason and should
   be revisited when the registry or server changes.
6. **Errors remain user-facing.** Diagnostics should say which server could not be installed,
   started, initialized, or stopped and why; raw exit codes alone are insufficient.
7. **No new external dependency is assumed.** If implementation reveals that a standards-compliant
   package-URL parser is necessary, adding a crate requires separate user approval.

## Remediation plan

### Phase 0: establish a reproducible baseline and failure taxonomy

1. Record the registry snapshot release tag/digest and the complete source ID for every selected
   pair in the workflow summary or an uploaded report.
2. Add a machine-readable result classification to the E2E harness or report generator. Suggested
   top-level categories are `install`, `launch`, `initialize`, `request`, `shutdown`, `exit`,
   `unsupported-platform`, and `external-download`.
3. Keep the existing human-readable diagnostic and failed-case footer; classification supplements
   it rather than replacing it.
4. Capture the current family totals above as the comparison baseline.

Pros:

- later phases can demonstrate which class they removed;
- genuine regressions are easier to distinguish from an upstream outage.

Cons:

- this adds reporting work before reducing the failure count;
- classifications must remain broad enough not to become a second server-specific manifest.

Acceptance criteria:

- every failed case has one stable top-level category and retains its detailed error;
- no failure is attributed only to a shard-level Cargo test exit code.

### Phase 1: define and implement explicit runtime staging

1. Model installer commands and runtime commands separately:
   - installer lookup uses the dedicated installation path;
   - server lookup uses only the isolated bin directory;
   - required runtime programs are linked into that isolated directory.
2. Derive baseline runtime requirements from installation families where the relationship is
   intrinsic:
   - npm launchers require Node unless the resolved artifact is independently executable;
   - PyPI launchers require the Python environment selected by the PyPI installation design;
   - NuGet apphosts continue to receive `DOTNET_ROOT` explicitly.
3. Continue supporting per-server `host-programs` for requirements not implied by the package
   family, such as `bash`, Java, compilers, or ecosystem CLIs.
4. Detect duplicate declarations and make staging idempotent.
5. Extend planner tests so runner setup and staged runtime requirements cannot drift apart.
6. Add regression tests which install or fake a script with `#!/usr/bin/env node` and prove that:
   - it starts when Node is a declared/derived runtime;
   - an unrelated ambient executable is still invisible.
7. Exercise representative npm and script-based GitHub servers in their existing playgrounds.

Pros:

- retains hermetic server selection;
- fixes the common npm failure at the correct boundary;
- supports future runtime-backed package families.

Cons:

- automatic family requirements can over-provision compiled packages distributed through npm;
- script dependencies which are not described by Mason still require manifest maintenance.

Rejected shortcut: append the complete host `PATH` to the server `PATH`.

- Advantage: minimal code and immediate coverage improvement.
- Disadvantages: an ambient LSP server may satisfy lookup, results become runner-dependent, and the
  documented guarantee that the downloaded server is tested no longer holds.

### Phase 2: preserve the minimum Cargo/rustup installer state

1. Determine which variables the selected CI toolchain actually requires. Expected candidates are
   `RUSTUP_HOME`, `CARGO_HOME`, and `RUSTUP_TOOLCHAIN`.
2. Thread only the required values into the isolated `lsp-cli` process and then the Cargo installer
   subprocess. Do not restore the complete ambient environment.
3. Keep Cargo out of the server runtime `PATH` unless a server explicitly needs it after launch.
4. Add unit tests for installer environment construction, including an isolated `HOME` and a fake
   rustup-style Cargo shim.
5. Add a regression test demonstrating that a Cargo package can install under an isolated home
   without first running `rustup default` inside that home.
6. Run representative Cargo-family cases from the ASM, Nix, and SystemVerilog playgrounds before a
   full Cargo-family workflow run.
7. Triage Cairo separately because its `scarb` executable mismatch will remain after this fix.

Pros:

- preserves environment isolation while allowing the configured toolchain to work;
- addresses almost the entire Cargo family with one generic mechanism.

Cons:

- rustup-specific variables introduce tool-manager awareness into the E2E harness;
- behavior must also be checked when Cargo is installed without rustup.

Alternative: stage a concrete `cargo` binary and compiler toolchain into the sandbox.

- Advantage: stronger isolation from the host environment.
- Disadvantages: substantially more setup logic, and staging only the Cargo executable is
  insufficient because Cargo also needs `rustc`, libraries, and toolchain metadata.

### Phase 3: parse Go subpaths as structured source metadata

1. Obtain representative current Mason source IDs containing `#cmd/...` and codify their expected
   interpretation in parser tests.
2. Extend the source representation with an optional decoded subpath/fragment rather than retaining
   it in the version.
3. Validate subpaths before joining them to the module path:
   - reject absolute paths;
   - reject parent traversal;
   - preserve valid nested command paths.
4. Construct the Go install argument as `<module>/<subpath>@<version>` when a subpath exists and as
   `<module>@<version>` otherwise.
5. Preserve existing qualifier handling and add tests combining encoded package names, versions,
   qualifiers, and fragments.
6. Prefer capabilities already available in the existing `url` dependency where appropriate. Do
   not add a package-URL crate without explicit permission.
7. Run the six currently failing Go shards, followed by the four already passing shards to guard
   against regression.

Pros:

- fixes the problem in the generic source model;
- leaves room for other backends to interpret subpaths deliberately.

Cons:

- package-URL fragment semantics must be confirmed rather than inferred from two examples;
- a generic source parser may expose fragment forms unsupported by a particular backend, requiring
  clear backend validation errors.

Acceptance criteria:

- Cue and Cuelsp installation commands have the subpath before `@version`;
- `gopls` and other fragment-free packages remain unchanged;
- malformed subpaths fail before invoking Go and name the invalid source ID.

### Phase 4: replace the broken PyPI prefix environment

Recommended approach: create a per-package virtual environment whose layout remains compatible with
the Mason package's expected `local/bin` paths, then install with that environment's Python.

1. Prototype `python3 -m venv <install-directory>/local` and verify the resulting executable paths
   against several current Mason PyPI package definitions.
2. Invoke pip through the environment's interpreter rather than ambient `python3 -m pip`.
3. Ensure generated entry points use the environment interpreter and can import their modules with
   an empty ambient `PYTHONPATH`.
4. Detect hosts where `venv`/`ensurepip` is unavailable and report a user-oriented prerequisite
   error.
5. Decide cache validity rules so an installation produced by the old prefix strategy is not reused
   as if it were a valid virtual environment. Prefer a receipt/schema marker over probing one known
   package.
6. Add compact parametrized unit tests for package/extras command construction and cached-layout
   validation.
7. Add regression coverage that starts a minimal installed console script, not merely checks that
   the script file exists.
8. Re-run all PyPI shards and the Python playground cases.
9. Update the existing Mason PyPI entry in `docs/GOTCHAS.md` with the resolved behavior.

Pros:

- uses Python's standard isolation mechanism;
- launchers naturally know their module search path;
- avoids Python-version-specific `site-packages` discovery in Rust.

Cons:

- virtual environments consume more space;
- some minimal Python installations omit `venv` or `ensurepip`;
- Mason's expected relative paths must be verified before committing to the layout.

Alternative: keep `pip --prefix` and generate wrappers which set `PYTHONPATH`.

- Advantage: smaller change to installation layout.
- Disadvantages: locating `site-packages` is Python-version/platform-dependent, wrappers need
  platform-specific handling, and subprocesses may lose the environment again.

Alternative: use `pip --target` and custom launchers.

- Advantage: predictable module destination.
- Disadvantages: console-script generation and dependency entry points become lsp-cli's
  responsibility, duplicating behavior already provided by virtual environments.

### Phase 5: triage remaining configuration and platform failures

After Phases 1-4, re-run the full matrix and review each remaining pair individually.

For every pair, choose exactly one disposition:

1. **Fix the generic data configuration** when Mason exposes a valid executable but the repository
   uses a placeholder or display name.
2. **Declare host requirements** when the downloaded server legitimately depends on a runtime or
   external tool which CI can provision generically.
3. **Add deterministic fixture setup** when the server requires a project-local configuration that
   can represent normal user setup without embedding secrets or machine-specific paths.
4. **Exclude the pair with a reviewed reason** when it requires unavailable proprietary tools,
   unsupported hardware/platforms, interactive configuration, or an upstream artifact which cannot
   currently run.
5. **Record a server gotcha** when initialization, request, or shutdown behavior is non-standard.
6. **Treat external download failures separately** and confirm recurrence before changing code or
   exclusions.

Do not encode language-specific behavior in lsp-cli to make a single test pass. Server-specific
setup belongs in E2E case data or a reviewed exclusion unless it reveals a reusable LSP behavior.

Pros:

- produces honest compatibility claims and high-signal failures;
- keeps exceptional setup visible and reviewable.

Cons:

- exclusions reduce tested breadth;
- maintaining many upstream servers is ongoing work, not a one-time code fix.

### Phase 6: resolve lifecycle failures without hiding leaked processes

1. For each remaining shutdown/exit failure, capture the initialize result, shutdown response,
   server stderr, process state, and whether cleanup killed a surviving process.
2. Compare behavior with LSP 3.17 requirements before changing client policy.
3. Separate these cases:
   - server closes instead of replying to `shutdown`;
   - server replies but ignores `exit`;
   - a child process retains pipes or remains alive;
   - client timeout accounting leaves too little shutdown budget.
4. Implement only generic protocol/process behavior in production code. Put verified server-specific
   limitations in `docs/GOTCHAS.md` and manifest exclusions.
5. Add regression tests using controlled fake servers for every generic policy change.
6. Confirm that a successful command leaves no server process, descendant, daemon socket, or
   retained isolated runtime directory.

Pros:

- protects normal CLI users from hangs and orphan processes;
- avoids accepting capability output while silently discarding cleanup failures.

Cons:

- strict protocol behavior may keep otherwise useful servers excluded;
- graceful-shutdown compatibility and bounded command latency are sometimes competing goals.

Potential alternative, requiring an owner decision: return successful query output after forcibly
cleaning up a server which mishandles shutdown, while emitting a warning.

- Advantage: users receive the requested data from more servers.
- Disadvantages: changes command success semantics, can normalize upstream protocol defects, and
  needs a reliable guarantee that forced cleanup succeeded.

### Phase 7: decide CI policy and encode it explicitly

The project owner must select one of these models.

#### Option A: green health gate (recommended)

- Run only supported and provisionable pairs as required jobs.
- Keep incompatible pairs as reviewed exclusions with reasons.
- Optionally run experimental pairs in a separate non-gating report.

Pros:

- a red workflow reliably signals a regression;
- branch protection and notifications remain useful.

Cons:

- compatibility breadth depends on disciplined exclusion review;
- excluded pairs can become compatible upstream without immediate detection unless an experimental
  audit also checks them.

#### Option B: intentionally permissive compatibility audit

- Attempt all candidate pairs.
- Publish structured results and do not fail the overall workflow for known pair failures.
- Fail only for planner/harness failures or changes relative to an approved baseline.

Pros:

- maximizes visibility into changing upstream compatibility;
- avoids maintaining a large exclusion list merely to keep GitHub green.

Cons:

- baseline comparison and result storage become essential;
- `continue-on-error` alone would mask regressions and is not an adequate implementation.

Whichever model is selected, document it in the newbie-facing `README.md` and the detailed E2E
guide. The workflow name, summary, and exit status should communicate the same meaning.

### Phase 8: verification and rollout

For each implementation phase:

1. Add or update compact unit tests in the affected Rust module. Review the entire test module for
   duplicated setup and extract parametrized helpers where needed.
2. Add a regression test for every confirmed defect.
3. Exercise the relevant real server against its existing `playground/` fixture as required by the
   project testing policy.
4. Run the narrow workflow selector first, for example:

   ```sh
   gh workflow run e2e.yml -f selector=installation-family -f value=cargo
   ```

5. Run the repository-wide test command:

   ```sh
   make test
   ```

6. Run the full compatibility workflow only after narrow cases pass.
7. Compare classified failure counts with the Phase 0 baseline and investigate every unexpected
   new failure or disappearance.
8. Land phases separately where practical so the nightly result identifies which change altered a
   compatibility class.

Final acceptance criteria:

- npm-backed servers receive their runtime without exposing ambient server executables;
- Cargo packages install under the isolated E2E home using the already provisioned toolchain;
- Go source subpaths produce valid install targets;
- PyPI launchers import their packages without ambient `PYTHONPATH`;
- all remaining unsupported pairs have reviewed, user-readable reasons;
- lifecycle failures are either fixed generically or documented and excluded;
- `make test` passes;
- representative cases pass in `playground/`;
- the full nightly has the owner-selected and documented success semantics.

## Proposed implementation order

The recommended order minimizes misleading downstream failures:

1. Phase 0: classification and baseline.
2. Phase 1: runtime staging.
3. Phase 2: Cargo/rustup environment.
4. Phase 3: Go subpath parsing.
5. Phase 4: PyPI environment.
6. Phase 5: pair-by-pair configuration/platform triage.
7. Phase 6: lifecycle policy and server gotchas.
8. Phase 7: final CI success policy.
9. Phase 8: full verification and rollout after every preceding phase.

Runtime and installer fixes come before manifest exclusions so the project does not classify pairs
as unsupported merely because the harness prevented their required runtime from starting.

## Risks and future limitations

- Mason registry metadata changes daily. Reproductions must preserve the registry source IDs and
  snapshot metadata used by the failing run.
- Installation family alone may not fully describe runtime dependencies. The design must allow
  package- or server-level additions without hardcoding a closed list.
- Tool-manager state is broader than executable lookup. Rustup is the current example; future
  installers may require their own homes, certificates, proxies, or SDK roots.
- A virtual-environment layout compatible with today's Mason links could change upstream. Receipts
  should record the installation strategy/version so stale layouts can be invalidated safely.
- Broad automatic retries can hide deterministic upstream breakage and increase CI time. Retry only
  errors classified as transient and keep the final cause visible.
- Exhaustive real-server testing will always have upstream flake and maintenance cost. A green gate
  and a broad audit may ultimately need to be separate workflows.
- No proposed fix should introduce language-specific parsing or source-code assumptions into
  lsp-cli.

## Decisions required from the project owner

These decisions were resolved by the user's subsequent instructions to implement the plan:

1. Use the green health gate, split between preferred PR coverage and exhaustive nightly/manual
   coverage.
2. Keep strict shutdown semantics; exclude servers which return data but cannot be cleaned up
   reliably.
3. Use standard-library per-package Python virtual environments.
4. Let installation families imply intrinsic runtimes while retaining explicit per-server host
   programs.
5. Keep the existing URL parsing dependency; no additional package-URL dependency was needed.

The original decision list follows for historical context.

Before Phase 7, and preferably before exclusions are changed, the owner needs to decide:

1. Should the nightly be a green health gate, a permissive compatibility audit, or two separate
   workflows?
2. May a query command succeed with a warning after forced cleanup when a server returned the
   requested data but violated shutdown/exit behavior?
3. Is a standard-library Python virtual environment acceptable as the managed PyPI installation
   layout, subject to validating Mason-relative paths?
4. Should installation families imply default runtime programs, or must every runtime remain an
   explicit manifest declaration?
5. If the existing `url` crate is insufficient for correct package-URL parsing, may a dedicated
   parser dependency be proposed? No dependency should be added without explicit approval.

No questions were asked during the original investigation. The later user instructions explicitly
requested implementation of the plan; the implemented choices are recorded above.

## Investigation difficulties

GitHub CLI returned no combined output for `gh run view --log-failed` on this unusually large matrix.
The investigation therefore used the paginated jobs API plus individual job logs and family-wide
checks. Counts and recurring installer signatures were verified directly; the GitHub/generic
examples are representative rather than a complete per-pair catalogue.
