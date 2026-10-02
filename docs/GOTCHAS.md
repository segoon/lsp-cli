# LSP protocol

## Capabilities

- Some servers send client requests such as `client/registerCapability` immediately after the
  `initialize` response and expect those requests to be answered before later client traffic.
  lsp-cli therefore drains and replies to requests already queued after `initialized` and before
  later outgoing operations. Requests that race with the next client request are answered while
  that request is outstanding, instead of assuming request-response traffic is strictly
  one-directional or waiting for a fixed post-initialization quiet period.
- After an initially empty `workspace/symbol` result, a `textDocument/didOpen` followed by a
  successful `textDocument/documentSymbol` response is a useful bounded signal that the server
  processed that document, but it does not prove that the workspace index is complete. OLS,
  Clojure LSP, and EmmyLua LS still returned no matching workspace symbols after this barrier in
  the pinned E2E fixtures, so workspace-symbol polling and their documented exceptions remain.
- A target-document `textDocument/publishDiagnostics` notification, completed `$/progress` item,
  or healthy quiescent `experimental/serverStatus` can shorten a workspace-symbol retry delay, but
  each is only a readiness hint. Consume at most one hint per query and retain bounded spacing for
  later retries; cached diagnostics must not collapse every remaining delay.
- LSP has no universal signal proving that an entire workspace index is complete. Servers verified
  not to expose a terminal `$/progress` or healthy quiescent `experimental/serverStatus` signal
  use the explicit `build-index-completion: best-effort` data policy. The client still observes the
  full bounded timeout and reports transport, protocol, server-status, and shutdown errors; only
  expiry without a terminal signal is accepted, so success must not be described as confirmation.


## Diagnostics

- Diagnostics are not uniformly query-shaped across servers.
  Some servers support client-initiated `textDocument/diagnostic`, while others only publish
  `textDocument/publishDiagnostics` asynchronously after `didOpen` or background analysis.
  A CLI diagnostics command therefore cannot rely on only one path if it wants broad coverage.
- `textDocument/publishDiagnostics` is latest-state data, not an append-only stream.
  Servers may replace older diagnostics for the same URI with a newer notification, including an
  empty list to clear prior errors. Keep only the latest notification per URI instead of treating
  each publish as an independent result item.
- A diagnostics command that only opens files and waits a short fixed delay is fragile.
  Some servers publish diagnostics only after preamble building, indexing, or other async work, so
  the client may need to wait through a bounded timeout budget instead of assuming diagnostics are
  available immediately after `didOpen`.
- Background-work progress such as `$/progress` is useful for diagnostics timing, but it is not a
  diagnostics result by itself. A server can report indexing activity without publishing any
  diagnostics yet, and some servers expose progress inconsistently, so progress should be treated as
  a hint rather than as the sole completion signal for `diag`.


# Daemon gotchas

- Daemon initialization compares workspace URIs literally. Directory URIs produced by
  `path_to_file_uri` end with `/`; raw socket clients and fixtures must use the same trailing
  slash in `rootUri` and workspace folder URIs, or initialization is rejected.

- LSP 3.17 assumes one server serves one tool. `lsp-cli daemon` therefore implements a
  conservative proxy policy instead of transparent multi-client sharing: only one client may be
  connected at a time, downstream `shutdown`/`exit` are handled locally, and a later client with
  different normalized `initialize` settings forces a fresh upstream server.
- `lsp-cli daemon` drops stale responses for disconnected clients and closes all client-owned
  documents on disconnect, but it does not persist dynamic registrations across sessions. If the
  upstream server uses `client/registerCapability` or `client/unregisterCapability`, lsp-cli marks
  the session as non-reusable and restarts the upstream server before the next client.
- Reused daemon sessions can attach after the upstream server has already finished indexing. To
  keep `wait-for-index` semantics stable for warm sessions, the daemon synthesizes a quiescent
  `experimental/serverStatus` notification after cached `initialize` replies when it already knows
  the upstream server is idle.
- Normal LSP commands opportunistically reuse the daemon socket. If the expected socket path exists
  but no daemon listens on it anymore, lsp-cli treats it as stale runtime state, removes the dead
  socket file, and falls back to starting a direct LSP server for that command.
- `stop` and `stop-all` use a private daemon control request over the Unix socket instead of normal
  LSP `shutdown`. This keeps regular client shutdown local to the proxy, but it also means `stop`
  only finds daemons whose socket path still matches the currently resolved workspace root and LSP
  command line; use `stop-all` when config changes make the exact match ambiguous.
- `workspace/applyEdit` only works while a client is actively connected. If no client is connected,
  lsp-cli rejects the request instead of editing files behind the client's back.
- During the LuaLS request-window benchmark, one `stop-all` cleanup reported an unexpected
  stop-response ID even though the daemon exited. Retrying against the same isolated runtime
  directory removed its stale socket. The unexpected response was not diagnosed; a failed stop
  response does not necessarily mean the daemon remains alive.
- Release profiling also reproduced a daemon exit after a successful query with
  `failed to write daemon client message: failed to write JSON-RPC message: Broken pipe`.
  Upstream notifications can race with client disconnect: the coordinator drains upstream
  traffic before client events, and a failed downstream write propagates out of `serve`.
  This can leave a stale socket and lose the indexed server between commands. Repeated commands
  must not be described as warm measurements without verifying process reuse. A regression
  test for a fix should close the client while an upstream notification is queued and verify
  that the daemon remains available to a subsequent client.

# LSP server implementations

## Exhaustive capability matrix

- Against Mason snapshot `2026-09-30-aboard-mob`, 100 capability cases could not produce a valid
  initialize response. They are stored as unavailable coverage and rendered `N/A`, with their
  earlier expected-failure diagnostics retained as exclusion reasons; they must not be presented
  as unsupported operations. Thirty-nine cases on 26 servers still fail during provisioning,
  before an LSP session exists, and remain expected failures because provisioning is validated
  independently of capability coverage.
- Twenty-nine cases on 23 servers historically completed initialization but did not exit within
  the command deadline after the standard `shutdown` response and `exit` notification:
  `bazelrc_lsp`, `buf_ls`, `circom-lsp`, `earthlyls`, `ginko_ls`, `gn_language_server`, `hyprls`,
  `jinja_lsp`, `jqls`, `jsonnet_ls`, `just`, `pest_ls`, `postgres_lsp`, `prosemd_lsp`, `regal`,
  `roc_ls`, `rumdl`, `solang`, `svls`, `terraformls`, `thriftls`, `tofu_ls`, and `ts_query_ls`.
  lsp-cli now gives an owned direct child up to one second to exit after that completed exchange,
  then terminates and reaps it without failing the completed operation. Their markers were removed
  and every affected case passed the final full-suite validation.
- Six more servers have distinct shutdown incompatibilities. `neocmake`, `helm_ls`,
  `markdown_oxide`, `v_analyzer`, and `gitlab_ci_ls` close the transport before the shutdown
  response is read; `csskit` rejects `shutdown` as an unknown method. These must remain separate
  from post-`exit` hangs because a tolerant cleanup policy cannot safely treat them identically.
- `glsl_analyzer` 1.7.1 and Verible `v0.0-4296-g0f262651` acknowledge `shutdown` and then close the
  transport before the client can send the required `exit` notification. The resulting broken
  pipe is distinct from both a missing shutdown response and a process that lingers after `exit`.
- Five initialize responses are not decodable as LSP: Bicep and Che4z COBOL emit output without a
  `Content-Length` header, Crystalline emits an invalid header, Foam returns boolean `false` where
  `CompletionOptions` are required, and NTT returns `null` where a sequence is required.
- Seven server configurations retain literal or unresolved paths: ElixirLS, GroovyLS,
  RakuNavigator, SQLS, Vespa LS, VHDL LS, and Visualforce LS. These are data/catalog defects, not
  evidence that their server implementations reject the protocol.
- Several otherwise installed servers depend on host tools hidden by the isolated E2E `PATH`.
  C3 LSP needs `c3c` and crashes when it is missing; Lexical needs `bash`; NextLS needs `elixir`;
  Reason Language Server invokes `uname`; Fish LSP needs `fish`; and Facility Language Server
  requires the .NET 6 runtime rather than the staged .NET 10 runtime.
- MDX requires `initializationOptions.typescript.tsdk`; BQLS requires a BigQuery project ID or
  `gcloud`; and Regols tries to resolve an empty filesystem path during initialization. These need
  explicit, data-driven configuration or dedicated fixtures rather than server-specific production
  branches.
- Three pinned releases are incompatible with the generic lane for upstream reasons: Hoon LS tries
  to reach a service at `127.0.0.1:80`, Solidity 0.8.37 reports that LSP support was removed, and
  SQL Language Server 1.7.1 imports a Node package subpath that is no longer exported.
- MesonLSP 5.0.4 times out during initialization. Templ 0.3.1020 closes during initialization
  without a diagnostic; its root cause remains unknown.

## Arduino Language Server

- Arduino Language Server 0.7.7 is not self-contained after installation. Without an explicit
  Arduino CLI configuration path it exits before initialization. A usable launch additionally
  needs Arduino CLI, `clangd`, a project-specific fully qualified board name (FQBN), and the
  matching board core installed through Arduino CLI.
- These coupled requirements belong in a dedicated managed-toolchain fixture if that operational
  dependency is approved, not in generic production LSP data. The current E2E setup can download
  one server and expose existing host programs, but cannot provision Arduino CLI plus `clangd` or
  run the mutable board-core installation. The pair remains excluded until a product decision
  accepts the additional network, storage, licensing, security, and maintenance surface.

## Astro Language Server

- Astro Language Server requires the TypeScript SDK location in `initializationOptions`.
  lsp-cli does not currently expose per-server initialization options, so the pair is explicitly
  excluded from executable capability coverage until that configuration is supported.

## Mason PyPI launchers

- PyPI packages must be installed into a virtual environment, not with `pip --prefix`. Prefix
  launchers resolve from the package directory but use the system interpreter, which cannot import
  the isolated package modules. lsp-cli creates a versioned per-package environment under `local/`
  and accepts it as cached only when its layout marker is present. This deliberately ignores old
  prefix launchers without deleting unrelated package installations.
- Mason `extra_packages` and data-provided `mason-extra-packages` must be installed in the same pip
  transaction as the primary package so pip resolves their combined constraints. The cache marker
  includes the source ID and dependency list; changing either rebuilds only that virtual
  environment instead of silently reusing an incompatible dependency set.

## Jedi Language Server

- Jedi Language Server 0.47.0 answers semantic queries but can fail to exit after the standard
  shutdown exchange. Its pygls worker may continue trying to write responses after stdout closes.
  Bounded direct-child cleanup now permits semantic smoke coverage without hiding a rejected or
  interrupted shutdown. Because the server exposes no terminal background-progress signal, its
  data selects bounded best-effort `build-index` semantics rather than confirmed completion.

## pylsp

- python-lsp-server 1.15.0 does not advertise `workspace/symbol`. It is covered by capability and
  provisioning checks, but is excluded from the source-language smoke suite because that suite uses
  workspace symbols to locate fixture declarations.

## PerlNavigator

- PerlNavigator 0.8.20 can return either an empty result or the sub itself when
  `textDocument/definition` is requested at a same-file sub declaration. The result differed
  between otherwise equivalent isolated local and CI runs and stayed empty across bounded local
  retries. Its E2E exception therefore requires a successful, well-formed response without
  asserting match cardinality.
- PerlNavigator's document-symbol responses for the playground expose declarations but not call
  sites, and the server does not advertise `textDocument/references`. Because `lsp-cli definition`
  currently accepts a name rather than a file position, it has no LSP-native way to retry this
  server at a stable use site. Removing the exception would require generic textual use-site
  discovery or a position-based public CLI interface; it cannot be solved by a fixture query
  override alone.

## Pyre

- Pyre requires a project configuration that declares `source_directories` or build targets before
  its persistent LSP command will initialize, and it requires a Watchman root marker. The Python
  playground carries minimal deterministic configuration for both. Pyre 0.9.25 then initializes
  but advertises only document synchronization, so it remains excluded from the semantic smoke
  suite rather than being mistaken for an installation failure.

## cmake-language-server

- cmake-language-server 0.1.11 declares `pygls>=1.1.1` but imports `LanguageServer` from the pygls
  1.x location. pygls 2 moved that API, so unconstrained installs fail during startup. The bundled
  LSP data adds `pygls<2` as a Mason extra package until upstream publishes a compatible release or
  upper bound. Provisioning and capability exchange pass with this constraint.

## RobotCode

- RobotCode 2.7.0 initializes but may remain alive after the standard shutdown/exit exchange.
  Bounded direct-child cleanup now permits capability coverage for both the Robot Framework and
  Resource language aliases while still requiring a successful shutdown response first.

## rpm-spec-language-server

- rpm-spec-language-server 0.0.2 depends on the PyPI `rpm` package, but that package is only a
  virtual-environment shim for an RPM Python extension already supplied by the operating system.
  It contains no native bindings. On the Ubuntu E2E host, provisioning succeeds and startup then
  fails while the shim searches for the absent system extension.
- RPM distributes its Python bindings with RPM itself, coupled to native RPM libraries and a
  compatible Python ABI. Copying files out of a Fedora/openSUSE package would also require staging
  its transitive native libraries and is not a portable hermetic PyPI installation. Upstream's
  container mode instead requires a container runtime and TCP transport, neither of which the
  current Mason installer or direct-process E2E model provides. The pair therefore remains
  excluded pending an explicit runtime/backend decision.

## salt-lsp

- salt-lsp 0.0.1 declares `PyYAML>=5.4,<6`. On the managed Python 3.12 runtime, pip finds no
  compatible PyYAML 5.4.1 wheel and its source build fails while determining wheel requirements.
  This is an upstream dependency/package-age incompatibility, not a launcher or LSP exchange
  failure. Supporting it would require an older Python runtime or an upstream/forked package with
  compatible dependencies; installing an extra PyYAML version cannot satisfy the declared `<6`
  constraint.

## textLSP

- textLSP imports GitPython during startup, and GitPython rejects an isolated `PATH` without the
  `git` executable. The E2E manifest declares Git as a host runtime rather than exposing the full
  ambient path.

## hdl-checker

- hdl-checker 0.7.5 enters its language-server mode without a `--lsp` argument; that old argument
  is rejected by its current command-line parser. The bundled server configuration deliberately
  invokes the executable without it.

## Esbonio

- Esbonio 2.x moved its stdio language server behind the `esbonio server` subcommand. Invoking the
  top-level command writes CLI help to stdout, which is not an LSP frame; the bundled configuration
  includes the required subcommand.

## pylyzer

- In an isolated current-Mason run against the Python playground, pylyzer returned no immediate
  `workspace/symbol` matches and no incoming call-hierarchy edges for `build_sample_order`, while
  its other applicable semantic queries completed. Exhaustive coverage records those two empty
  results explicitly instead of treating advertised capabilities as a guarantee of fixture matches.
- Mason package `pkg:github/mtshiba/pylyzer@v0.0.82` also returns no
  `textDocument/typeDefinition` match for an `order` parameter explicitly typed as `Order`, after
  the same fixture passes that query with BasedPyright and pyright. This is a pylyzer compatibility
  limitation rather than a missing fixture relationship. Separately, the server repeatedly reports
  a missing `ERG_PATH` and diagnostics-worker index-out-of-bounds panics; semantic replies can still
  succeed, so those stderr messages are not currently treated as the query's direct failure.

## basedpyright

- Mason package `pkg:pypi/basedpyright@1.40.1` returned no `textDocument/implementation` match when
  the shared profile incorrectly queried the plain function `build_sample_order`. After the fixture
  gained an abstract `OrderFormatting` interface and the E2E profile selected it specifically for
  implementation queries, the complete smoke case passed twice and its marker was removed.

## pyright

- In an isolated current-Mason run against the Python playground, pyright advertised workspace
  symbols but returned no matches before background analysis completed. It also exposed no
  background-work completion signal usable by `build-index`; workspace queries retain bounded
  readiness polling and indexing uses the explicit best-effort policy instead of a fixed sleep.
- pyright does not implement `$/progress`/`workDoneProgress` or `experimental/serverStatus` over
  the language server protocol at all (it only reports progress via its separate CLI's
  `--outputjson` mode), so `wait_for_background_work` has no protocol signal to key off for this
  server specifically. A pyright-only readiness heuristic would need to open the target document
  and treat the first `textDocument/publishDiagnostics` for that URI as "analyzed enough", since
  pyright reliably publishes diagnostics (even an empty list) once it has actually checked a file.
  That is a per-server heuristic, not a real completion signal: it cannot distinguish "not analyzed
  yet" from "analyzed with zero diagnostics" until the notification actually arrives, so it still
  needs a bounded timeout and remains unverified against pyright's actual behavior.

## zuban

- Current-Mason Zuban answers the Python playground's semantic queries but exposes no
  background-work progress signal usable by `build-index`. Its data selects bounded best-effort
  semantics and does not claim that indexing has completed.

## ty

- Current-Mason ty returned an empty callers result in one isolated Python fixture run and a
  non-empty result in the next otherwise identical run. The exhaustive pair remains excluded until
  call-hierarchy results are deterministic enough for a stable expectation.

## Pyrefly

- Current-Mason Pyrefly initializes against the Python playground but returns no workspace
  symbols, document symbols, or document functions. The pair is excluded because later named
  queries cannot be given a meaningful semantic assertion without a discoverable symbol.

## gopls

- gopls advertises `implementationProvider`, but `textDocument/implementation` on a free function
  fails with a server error explaining that the symbol is a function rather than a method. E2E
  coverage records that bounded failure for fixtures whose shared callable query is a free
  function; a non-empty implementation assertion needs an interface type or method fixture.

## deno lsp

- The current Mason Deno server initializes for JavaScript and TypeScript, but rejects the LSP
  `shutdown` request when its parameters are `null`, returning `-32602` and asking for non-null
  parameters. LSP 3.17 defines `shutdown` with no parameters, so direct lsp-cli commands currently
  report an unclean shutdown and the Deno pairs remain excluded.

## typescript-language-server

- Version 4.4.0 with TypeScript 6.0.3 advertises `workspaceSymbolProvider`, but a fresh
  `workspace/symbol` request can fail with `No Project` before any document has been opened. A
  capability-aware test must distinguish this advertised-but-not-yet-ready behavior from an
  unsupported capability.
- The server exposes no background-work progress notification usable by `build-index`; its data
  selects bounded best-effort semantics instead of sleeping or assuming project analysis completed.

## vtsls

- In isolated current-Mason runs against the JavaScript and TypeScript playgrounds, vtsls returned
  no immediate workspace-symbol matches before project analysis completed and exposed no
  background-work completion notification usable by `build-index`. Workspace-symbol retries and
  the data-driven best-effort indexing policy keep both paths bounded without a fixed delay.

## jdtls

- The current Mason jdtls launcher requires Java 21 or newer. Merely resolving a `java` executable
  is insufficient: GitHub's default Java may be older and makes the launcher exit before the LSP
  `initialize` response. CI must provision Java 21 explicitly before enabling the lifecycle case.
- The current Mason jdtls package needs both Java to run and Python to install its launcher.
  Bounded cleanup handles its direct-process lifetime after a successful exchange, but shared
  semantic smoke remains excluded: method symbols are decorated rather than matching the fixture
  names, and asynchronous indexing makes references and callers intermittently empty. The raw
  direct lifecycle scenario also exits with status 1 after acknowledging shutdown, so it remains
  excluded independently of the post-`exit` cleanup policy.
- `stop` removes a jdtls daemon socket before the upstream Java process has necessarily completed
  shutdown. Immediately starting another jdtls for the same workspace can overlap the old process
  and stall initialization. Lifecycle tests wait, with a deadline, for the recorded upstream PID
  to exit after `stop`; socket disappearance alone does not prove complete process termination.

## kotlin-language-server

- The current Mason Kotlin Language Server launcher also needs `uname` and `xargs` in its isolated
  path. With those tools present it initializes and answers the request, but the process does not
  exit before the direct-run deadline after shutdown. Its exhaustive pair remains excluded until
  direct shutdown is reliable.

## kotlin-lsp

- The Mason package exposes Kotlin LSP as `intellij-server`, not `kotlin-lsp`. The data config must
  use the packaged launcher name so `--download` can resolve it; a display name or upstream product
  name is not necessarily an executable name.
- The Mason generic package uses `{{ version | strip_prefix "kotlin-lsp/v" }}` in its download URL
  and executable path. Mason version prefixes are package-specific, so lsp-cli's template renderer
  supports quoted `version | strip_prefix "<literal>"` expressions rather than special-casing the
  Kotlin prefix. Unsupported filters remain unresolved instead of being guessed.
- Mason version `kotlin-lsp/v262.9593.0` downloaded and launched, but `intellij-server` reported
  that the build had expired before completing LSP initialization. The registry later moved to
  `kotlin-lsp/v263.4702.0`, which initializes and completes a direct shutdown exchange.
- Version `kotlin-lsp/v263.4702.0` starts Gradle synchronization and may download a Gradle
  distribution during a cold start. It reports terminal work-done progress, but still returns no
  workspace-symbol or reference matches for the shared Kotlin fixture. A longer progress wait does
  not repair those semantic results, so semantic smoke remains excluded while lifecycle coverage
  runs independently.

## roslyn-language-server

- The Mason package exposes Roslyn through the `roslyn-language-server` .NET tool launcher. A data
  config containing a literal installation placeholder cannot work with generic `--download`;
  launch the Mason-exposed command and let the NuGet backend manage its concrete installation path.
- Roslyn 5.12 returns namespace-qualified type names and parenthesized method names from
  `textDocument/documentSymbol`. E2E schema v13 permits pair-local exact callable and expected-name
  overrides so tests retain Roslyn's output without changing production results or weakening
  OmniSharp assertions. Roslyn still returns no workspace symbols, references, or call-hierarchy
  edges for the shared fixture and exposes no terminal background-index signal; those results are
  asserted as narrow query exceptions.

## OmniSharp

- OmniSharp 1.39.15 previously closed the transport while lsp-cli waited for a `shutdown` response.
  Against pinned Mason snapshot `2026-09-30-aboard-mob`, however, the complete C# smoke case passed
  twice in fresh isolated homes, including repeated direct shutdown exchanges. Its stale broad
  expected-failure marker was removed; the dedicated lifecycle scenario remains the narrower place
  to detect a recurrence.

## svls

- Current-Mason SVLS 0.2.14 installs and reaches the end of a direct capability query, but the
  server does not exit before the post-`shutdown` deadline. Treat this as a lifecycle failure; a
  successful Cargo installation does not make the direct-process case cleanly terminable.

## jq-lsp

- Current-Mason jq-lsp 0.1.18 installs and completes a direct capability query, but does not exit
  before the post-`shutdown` deadline. Keep this lifecycle behavior distinct from Go package
  installation success.

## jsonnet-language-server

- Current-Mason jsonnet-language-server 0.17.0 initializes for both Jsonnet and Libsonnet, but does
  not exit before the post-`shutdown` deadline in direct capability runs. Its stderr reaches normal
  initialization and reports no shutdown-specific explanation.

## regols

- Current-Mason regols 0.2.4 installs and starts, but rejects initialization for the committed Rego
  playground with `lstat : no such file or directory`. The empty path originates in the server;
  the required workspace-layout or initialization expectation has not yet been established.

## emmylua_ls

- Current-Mason EmmyLua answers semantic requests for the Lua playground, but its formatting
  response contains a line outside the requested file. lsp-cli correctly rejects that invalid edit;
  exhaustive coverage records the stable user-facing failure.
- The same release returned no immediate workspace-symbol matches. It also returned no outgoing
  call-hierarchy edges for both an explicit local-function call (`format_timestamp` to
  `render_timestamp`) and an annotated concrete method call (`format` to `normalize_timestamp`).
  The callee exception is therefore a server limitation rather than an absent fixture edge; it is
  not evidence that call hierarchy is universally unsupported.
- Mason package `pkg:github/CppCXY/emmylua-analyzer-rust@0.25.1` returned no
  `textDocument/implementation` match when the profile incorrectly queried the plain
  `format_timestamp` function. With an annotated base formatter method, a derived implementation,
  and a command-specific query target, the complete smoke case passed twice and its broad marker
  was removed.

## lua-language-server

- Opening every source file for symbol discovery also triggers diagnostics. The installed
  LuaLS `textDocument/didOpen` handler opens and compiles the file; its diagnostic provider
  runs diagnostics on file-open events. In a release-build `parley.nvim` experiment with 193
  Lua files and a request window of 20, an explicit temporary configuration with diagnostics
  enabled took 12.57/12.37 seconds; disabling only diagnostics took 4.50/2.84 seconds with
  identical reference matches. Traces showed zero diagnostic notifications in the disabled
  case. This is a server-specific experimental control, not a recommendation to silently
  disable diagnostics for normal commands or shared sessions.
- In a September 2026 investigation against `parley.nvim`, `workspace/symbol` returned null for
  the local function `normalize_timestamp`, while `textDocument/documentSymbol` exposed it and
  `textDocument/references` found its call site. Workspace-symbol results alone therefore cannot
  replace document-symbol discovery for this query.
- Two direct-process runs in that investigation completed their queries but failed while waiting
  for LuaLS to exit, adding the configured 30-second timeout. The debug trace showed a successful
  `shutdown` response followed by an `exit` notification. Bounded direct-child cleanup now makes
  direct queries and lifecycle coverage reliable for this condition. LuaLS selects best-effort
  `build-index` because it exposes no terminal signal proving workspace indexing is done.

## rust-analyzer

- `rust-analyzer` may answer `textDocument/documentSymbol` with flat `SymbolInformation` items whose
  `location.range.start` points at the start of the whole declaration (for example `pub fn` or an
  attribute line) instead of the identifier itself. When a later LSP request needs a precise
  symbol position, prefer recovering the identifier offset from source text inside that range
  instead of assuming `range.start` is directly queryable.
- `rust-analyzer` package source `pkg:github/rust-lang/rust-analyzer@2026-09-21` returned non-empty
  outgoing call-hierarchy results for the Rust playground's `sample_order` in four consecutive
  isolated runs. Although the function primarily constructs data, it invokes methods such as
  `to_string`; do not classify constructor-heavy fixture functions as having no callees without
  checking the server's current call-hierarchy interpretation.
- Those outgoing call-hierarchy results depend on the Rust standard-library sources being
  available. The same rust-analyzer release initialized in CI without the `rust-src` toolchain
  component, reported that it could not load the standard library, and returned no callees for
  `sample_order`. Install `rust-src` when a test expects calls into the standard library, or use a
  fixture whose expected call edges stay within the workspace.
- rust-analyzer resolves the new `OrderTotaling` trait implementation, but still returns no
  `textDocument/typeDefinition` result for the top-level `SAMPLE_ORDER_VALUE` static explicitly
  declared as `Order`. That retained exception is a server/query compatibility limitation, not an
  absent fixture type relationship.

## Specialized capability-only servers

- `htmx-lsp` 0.1.0 logs the standard `shutdown` request as unhandled and never answers it;
  `efm-langserver` 0.0.57 closes the transport before its shutdown response reaches the client.
  These are server lifecycle deviations, not evidence that generic shutdown should be weakened.
- `bacon-ls` 0.31.0 rejects a standard initialize request as JSON-RPC invalid-request. `ast-grep`
  0.45.3 refuses to initialize without an ast-grep project configuration.
- Grammarly Language Server 0.0.4 requires a `clientId` initialization option, and GitHub Actions
  Language Server 0.3.61 requires a `sessionToken`. Supplying invented credentials would turn a
  protocol check into an inaccurate product configuration, so both remain expected failures.
- Angular Language Server 22.2.0 needs valid TypeScript and Angular probe locations; the bundled
  command currently passes option names where locations are expected. The LWC config similarly
  contains a literal `/path/to/node_modules/...` placeholder. Both need data fixes, not generic
  language-specific production branches.
- The pinned Mason snapshot has packages whose configured executable cannot be installed or
  resolved for `nextflow_ls`, `spectral`, and `stylelint_lsp`. Keep these as explicit provisioning
  and capability expected failures until either the catalog command or Mason recipe is corrected.
- The pinned Mason snapshot has no package for `debputy`, `fortitude`, `gitlab_duo`, `selene3p_ls`,
  `sqruff`, `statix`, `stylua3p_ls`, `theme_check`, `ttags`, `turbo_ls`, `tvm_ffi_navigator`, or
  `vacuum`. Keep them provisioning-excluded rather than assigning capability coverage: the
  workflow planner cannot infer an installation family for a package that does not exist.
- LTeX and LTeX Plus launch through `/usr/bin/env sh` and call standard shell utilities. Their
  isolated cases must stage `sh`, `dirname`, `uname`, and `which`; LTeX also needs host `java`,
  while LTeX Plus includes its own JDK.
- Shopify Theme Language Server can briefly recreate Node's compile cache after the client exits,
  racing immediate sandbox removal. One 53-server run retained only that cache; an isolated retry
  cleaned up successfully, so no expected-failure marker was added.

## clangd

- `clangd` may start successfully without sending background-work progress notifications. Its
  `build-index` data therefore selects best-effort completion. Keep normal symbol-query configs on
  `wait-for-index: false` unless progress reporting is confirmed for the target `clangd` setup;
  explicit symbol-query waits retain confirmed semantics.
- `compile_commands.json` requires absolute working directories, which makes a committed database
  stale when an E2E project is copied. Portable clangd fixtures should use `compile_flags.txt` or
  generate the database after copying instead of committing checkout-specific paths or a relative
  `directory` value.
- clangd 22.1.6 returned document symbols, definitions, references, and call hierarchy for the
  CUDA, Objective-C, and Objective-C++ playgrounds, but an immediate `workspace/symbol` query
  returned no matches. It also exposed no progress signal usable by `build-index`; its explicit
  bounded best-effort policy avoids a fixed indexing sleep without claiming completion.
- clangd 23.1.0 reproduced that race for Objective-C++ when an implementation-query interface was
  declared only in a `.hpp` file outside the `objcpp` fixture's `.mm` document scan: targeted runs
  could pass while the full parallel suite fell back to an empty immediate `workspace/symbol`
  result. The fixture keeps relationship anchors in a scanned source file so the assertion does
  not depend on background-index timing.
- `clangd` may expose diagnostics only through delayed `textDocument/publishDiagnostics` even when
  it does send `$/progress`, and in some setups it does not advertise `diagnosticProvider` for
  pull diagnostics at all. For `lsp-cli diag`, prefer pull diagnostics when the capability is
  advertised, but keep a timeout-bounded push fallback for `clangd`-style behavior.
