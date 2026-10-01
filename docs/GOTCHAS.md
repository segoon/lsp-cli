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

- Against Mason snapshot `2026-09-30-aboard-mob`, all 118 capability cases carrying an
  expected-failure marker failed again. Thirty-nine cases on 26 servers fail during provisioning,
  before an LSP session exists; those results must not be classified as protocol incompatibility.
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

- Arduino Language Server is not self-contained after installation. It requires Arduino CLI
  configuration, an installed board core, `clangd`, and a project-specific fully qualified board
  name (FQBN) before initialization. The generic E2E fixture therefore records a reviewed
  exclusion instead of treating missing machine/project configuration as a server regression.

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
  `build-index` case retains a narrow exception rather than claiming confirmed index completion.

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

- The PyPI package imports the system RPM Python module. Installing it in an isolated virtual
  environment is not sufficient on hosts without compatible RPM bindings, so the current CI pair
  is explicitly excluded.

## salt-lsp

- salt-lsp 0.0.1 cannot currently complete a pip installation on the CI Python runtime. The pair is
  classified as an upstream package-install incompatibility rather than a launcher/import failure.

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
  background-work completion signal usable by `build-index`; tests assert both bounded outcomes
  instead of adding a fixed indexing sleep.
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
  background-work progress signal usable by `build-index`. Exhaustive coverage asserts the bounded
  user-facing failure rather than inferring that indexing has completed.

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
- The server exposes no background-work progress notification usable by `build-index`; assert the
  bounded user-facing failure instead of sleeping or assuming that project analysis completed.

## vtsls

- In isolated current-Mason runs against the JavaScript and TypeScript playgrounds, vtsls returned
  no immediate workspace-symbol matches before project analysis completed and exposed no
  background-work completion notification usable by `build-index`. Exhaustive coverage records
  both bounded outcomes instead of relying on a fixed indexing delay.

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
- Mason version `kotlin-lsp/v262.9593.0` downloads and launches, but `intellij-server` reports that
  the build has expired and exits before completing LSP initialization. Detection and file listing
  still work, and a daemon can create its socket and be stopped, but capability and semantic checks
  are blocked until the registry provides a usable build.

## roslyn-language-server

- The Mason package exposes Roslyn through the `roslyn-language-server` .NET tool launcher. A data
  config containing a literal installation placeholder cannot work with generic `--download`;
  launch the Mason-exposed command and let the NuGet backend manage its concrete installation path.

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
  direct queries and lifecycle coverage reliable for this condition. `build-index` still retains
  a narrow exception because LuaLS exposes no terminal signal proving workspace indexing is done.

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

## clangd

- `clangd` may start successfully without sending the background-work progress notifications that
  `lsp-cli` currently expects for `wait-for-index`/`build-index` flows. Keep normal symbol-query
  configs on `wait-for-index: false` unless that progress reporting is confirmed for the target
  `clangd` setup.
- `compile_commands.json` requires absolute working directories, which makes a committed database
  stale when an E2E project is copied. Portable clangd fixtures should use `compile_flags.txt` or
  generate the database after copying instead of committing checkout-specific paths or a relative
  `directory` value.
- clangd 22.1.6 returned document symbols, definitions, references, and call hierarchy for the
  CUDA, Objective-C, and Objective-C++ playgrounds, but an immediate `workspace/symbol` query
  returned no matches. It also exposed no progress signal usable by `build-index`; capability-aware
  tests need an explicit bounded policy rather than a fixed indexing sleep.
- clangd 23.1.0 reproduced that race for Objective-C++ when an implementation-query interface was
  declared only in a `.hpp` file outside the `objcpp` fixture's `.mm` document scan: targeted runs
  could pass while the full parallel suite fell back to an empty immediate `workspace/symbol`
  result. The fixture keeps relationship anchors in a scanned source file so the assertion does
  not depend on background-index timing.
- `clangd` may expose diagnostics only through delayed `textDocument/publishDiagnostics` even when
  it does send `$/progress`, and in some setups it does not advertise `diagnosticProvider` for
  pull diagnostics at all. For `lsp-cli diag`, prefer pull diagnostics when the capability is
  advertised, but keep a timeout-bounded push fallback for `clangd`-style behavior.
