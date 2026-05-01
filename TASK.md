## Task

Implement `--download` for `lsp-cli detect`.

Behavior:
- keep current detection logic
- if the detected LSP executable is already available in `$PATH`, keep using it
- otherwise, if `--download` is passed, download/install the LSP server and return a runnable command
- focus on LSP servers only
- aim to cover Mason main cases first, not necessarily every exotic package backend on day one

Reference source:
- Mason registry from GitHub releases
- use `registry.json.zip`, not a vendored snapshot and not a git clone


## Agreed Decisions

- Do not vendor Mason registry data in this repo.
- Do not clone the Mason registry git repository.
- Download the latest published `registry.json.zip` at runtime and cache it for future invocations.
- Keep writable runtime state separate from config data.
- Build the installer logic to be reusable, but expose it through `detect --download` first.


## Current Status

Already implemented:
- `detect --download` CLI parsing
- `detect` integration with Mason-aware executable resolution
- current resolution priority:
  - use executable from `$PATH` when available
  - otherwise use already managed local install when available
  - otherwise install when `--download` is passed
- writable runtime state root at `~/.local/share/lsp-cli`
- runtime directories:
  - `registry/`
  - `packages/`
  - `bin/`
  - `share/`
  - `receipts/`
- Mason registry refresh from GitHub latest release metadata
- download and unpack of `registry.json.zip`
- digest verification when GitHub release metadata provides one
- 30-day freshness checks for cached registry data
- stale-cache fallback with warning when refresh fails
- parsing of Mason `registry.json`
- filtering to Mason packages in `LSP` category
- primary mapping from `data/lsp/<stem>.yaml` to Mason `neovim.lspconfig`
- conservative explicit mapping overrides for selected historical names
- exact fallback lookup by Mason package name
- exact fallback lookup by uniquely-exposed Mason binary name
- source-id parsing for phase 1 backends:
  - `github`
  - `npm`
  - `pypi`
  - `cargo`
  - `golang`
  - `generic`
- decoding of URL-encoded Mason package names in `source.id` (for scoped npm packages such as `%40angular/...`)
- platform target matching
- template interpolation subset for common Mason expressions
- selected extra live-registry template fields:
  - `{{source.asset.ext}}`
  - `{{source.download.man}}`
  - named asset-bin templates such as `{{source.asset.bin.kcl_language_server}}`
- source-level `version_overrides` support for current Mason `semver:<=...` rules
- install backends for:
  - `github`
  - `npm`
  - `pypi`
  - `cargo`
  - `golang`
  - `generic`
- wrapper/direct executable support for:
  - direct executable paths
  - `npm:`
  - `pypi:`
  - `cargo:`
  - `golang:`
  - `exec:`
  - `python:`
  - `dotnet:`
  - `java-jar:`
  - `node:`
- `share` materialization support
- install receipts
- managed executable resolution reuse in `run`
- unit tests for CLI parsing, runtime state, registry parsing, source parsing, platform matching, template rendering, link/wrapper resolution, and detect resolution

Partially implemented:
- runtime state is separated logically from config loading, but both still default under `~/.local/share/lsp-cli`
- mapping strategy currently implements only the primary `lspconfig` mapping
- mapping overrides and fallback matching are intentionally conservative and exact-only
- Mason schema/template support is still intentionally partial, but now covers the live LSP cases hit during manual verification (`ast-grep`, `quick-lint-js`, named asset bins)
- `share` entries are materialized by copying files/directories, not linking
- receipts are written but not yet used for lookup or validation
- current wrapper/install flow is primarily Unix-oriented

Still to implement:
- `opt`-style link handling if needed by real packages
- reuse of managed executable resolution in later query/index commands if useful
- more Mason template/schema coverage beyond the currently supported live LSP fields
- broader end-to-end tests for registry refresh and install flows
- broader manual verification beyond the currently checked `python`, `typescript`, and `rust` playgrounds

Important consequence:
- the original first vertical slice is effectively done already
- remaining work is now mostly coverage, compatibility, and reuse rather than initial architecture


## Recommended Runtime Layout

Reason:
- current config may be loaded from repo `data/`, which must stay read-only
- downloaded registry data and installed packages need a stable writable location

Recommended writable state root:
- `~/.local/share/lsp-cli/` by default

Suggested layout:
- `registry/registry.json`
- `registry/metadata.json`
- `packages/<package>/...`
- `bin/`
- `share/`
- `receipts/<package>.json`


## Registry Refresh Plan

Use GitHub release metadata endpoint:
- `https://api.github.com/repos/mason-org/mason-registry/releases/latest`

From that response:
- locate asset `registry.json.zip`
- use `browser_download_url`
- use `digest` when available for integrity verification

Refresh policy:
- if cached registry is fresh enough, use it
- if older than threshold, check latest release metadata
- if release tag changed, download and unpack new `registry.json.zip`
- if tag did not change, just update local refresh timestamp

Recommended initial threshold:
- `30 days`

Fallback behavior:
- if refresh fails but cached registry exists, use stale cache and warn
- if refresh fails and no cached registry exists, fail clearly

Pros:
- much simpler than cloning/parsing package YAMLs
- no `git` runtime dependency
- close to Mason's published compiled artifact

Cons:
- depends on GitHub release/API availability
- tied to compiled registry schema


## Scope For Phase 1

Implement enough Mason-like logic to cover the main LSP cases.

Install source backends to support first:
- `github` release assets
- `npm`
- `pypi`
- `cargo`
- `golang`
- `generic` direct downloads

These cover most LSP packages in Mason.

Link/wrapper support needed in phase 1:
- direct executable paths
- `npm:` wrappers
- `pypi:` virtualenv binaries
- `cargo:` binaries
- `golang:` binaries
- `exec:` wrappers
- `dotnet:` wrappers
- `python:` wrappers
- `share` links

Main practical servers expected to work after phase 1:
- `pyright`
- `typescript-language-server`
- `bash-language-server`
- `clangd`
- `gopls`
- `rust-analyzer`
- `lua-language-server`
- `jdtls`


## Later Phases

Phase 2:
- `github` build recipes
- `generic` build recipes
- broader template-expression coverage
- more wrapper variants if needed

Phase 3:
- `openvsx`
- `gem`
- `nuget`
- `luarocks`
- `opam`
- `composer`

Reason for deferral:
- these are rarer for LSP use
- they add ecosystem-specific complexity
- build-based installs are less predictable than prebuilt downloads


## Mapping Strategy

Primary mapping:
- `data/lsp/<stem>.yaml` -> Mason `neovim.lspconfig`

Secondary mapping:
- explicit overrides for known mismatches

Tertiary fallback:
- careful package-name or executable-name matching for selected known cases only

Reason:
- direct overlap already covers most useful cases
- name-only matching is too risky as a global rule

Risk:
- some `lsp-cli` configs will not map directly to Mason packages and must fail clearly until an override is added

Current status:
- primary mapping is implemented
- explicit overrides are implemented for a small conservative alias set
- exact package-name and unique binary-name fallback is implemented
- broad fuzzy matching is intentionally not implemented


## Minimal Mason Logic Worth Re-implementing

- parse `registry.json`
- filter to packages where `categories` contains `LSP`
- index by:
  - `neovim.lspconfig`
  - package name
- parse purl `source.id`
- choose platform target, e.g.:
  - `linux_x64_gnu`
  - `linux_x64_musl`
  - `darwin_arm64`
  - `win_x64`
- apply `version_overrides`
- interpolate the common template expressions used by Mason LSP specs:
  - `{{version}}`
  - `{{ version | strip_prefix "v" }}`
  - `{{source.asset.bin}}`
  - `{{source.asset.file}}`
  - `{{source.download.config}}`
- expand `bin`, `share`, and `opt`-style link data as needed

Decision:
- do not try to clone Mason wholesale
- implement only the subset required by supported LSP packages

Consequence:
- phase 1 stays smaller and more testable
- unsupported cases must return explicit user-facing errors

Current status:
- implemented:
  - parse `registry.json`
  - filter LSP packages
  - index by `neovim.lspconfig`
  - index by package name and unique binary name for conservative fallback
  - parse supported source-id kinds needed for phase 1
  - choose platform target
  - apply current `semver:<=...` `version_overrides`
  - interpolate the current common template subset plus selected live-registry extras
  - expand `bin` and `share`
- not implemented yet:
  - `opt` handling


## Proposed Rust Module Split

Suggested new modules or equivalents:
- `runtime_state.rs`
  - resolve writable state root
  - create cache/install directories
- `mason_registry.rs`
  - refresh/load cached `registry.json`
  - parse registry entries
  - build LSP indexes
- `mason_purl.rs`
  - parse Mason package URLs
- `mason_platform.rs`
  - resolve current platform target
- `mason_template.rs`
  - template interpolation subset
- `mason_install.rs`
  - orchestrate installation
- `mason_install/backends/*.rs`
  - `github`, `npm`, `pypi`, `cargo`, `golang`, `generic`
- `mason_link.rs`
  - wrappers and link expansion

This split is only a recommendation.

Pros:
- separates registry, platform, install, and wrapper concerns
- easier unit tests

Cons:
- more files and types
- may be over-structured if the implementation stays small

Minimal alternative:
- keep more logic together in one or two modules initially


## CLI / Behavior Changes

Add to `detect`:
- `--download`

Expected behavior for `lsp-cli detect --download <path>`:
1. detect filetypes and matching LSP suggestions
2. for each selected suggestion:
   - use `$PATH` executable if available
   - else use already managed local install if available
   - else install when `--download` is set
3. print final command with the resolved executable path

Important user-facing rule:
- errors should describe the problem from the user's point of view
- do not dump low-level internal details unless unavoidable

Examples of good errors:
- `cannot install pyright because npm is not available in $PATH`
- `cannot install lua-language-server on this platform`
- `no Mason install recipe is available for detected server <name>`

Current status:
- `detect --download` is implemented
- `detect` already resolves to:
  - `$PATH` executable first
  - then managed local install
  - then installation when `--download` is enabled
- equivalent managed resolution is now wired into `run`


## Remaining Implementation Order

Already done:
1. Add `--download` to CLI and detect command output path.
2. Add writable runtime state root resolution.
3. Add cached registry metadata format and freshness checks.
4. Implement download + unpack of `registry.json.zip`.
5. Parse and index LSP entries from `registry.json`.
6. Implement primary `lsp-cli` config -> Mason package mapping.
7. Implement platform matching and minimal template interpolation.
8. Implement install backends for:
   - `github`
   - `npm`
   - `pypi`
   - `cargo`
   - `golang`
   - `generic` download
9. Implement wrapper/share logic.
10. Integrate executable resolution into `detect --download`.

Recommended next order:
1. Decide whether to expand the explicit override table beyond the current conservative aliases.
2. Decide whether broader fallback matching is worth the risk, or if exact-only fallback should remain the limit.
3. Add broader tests for registry refresh/install flows where practical.
4. Verify manually in additional `playground/` targets such as `go`, `c`/`cpp`, and `java`.
5. Run:
   - `cargo test -q`
   - `cargo clippy`


## Testing Plan

Unit tests should cover:
- CLI parsing for `detect --download`
- registry metadata freshness logic
- registry parsing and LSP filtering
- `lspconfig` mapping
- purl parsing
- platform target selection
- template interpolation subset
- install plan resolution for representative packages
- wrapper path generation
- stale-cache fallback behavior

Current status:
- already covered in unit tests:
  - CLI parsing for `detect --download`
  - registry metadata freshness logic
  - registry parsing and LSP filtering
  - primary `lspconfig` mapping
  - conservative mapping overrides and exact fallback lookup
  - purl/source-id parsing for supported backends
  - URL-decoding of Mason package names in `source.id`
  - platform target selection
  - current `version_overrides` handling
  - template interpolation subset
  - selected live-registry template/schema cases:
    - `source.asset.ext`
    - `source.download.man`
    - named asset-bin mappings
  - wrapper path generation and cached wrapper resolution
  - managed resolution reuse in `run`
- still missing or weak:
  - registry refresh/download behavior against realistic HTTP responses
  - end-to-end install-plan coverage for representative real packages
  - broader manual `playground/` verification beyond `python`, `typescript`, and `rust`

Manual checks in `playground/` should cover at least:
- `playground/python`
- `playground/rust`
- `playground/typescript`
- `playground/go`
- `playground/c` or `playground/cpp`
- `playground/java` if `jdtls` is included in phase 1


## Good First Milestone

Good first vertical slice:
- implement registry cache download/load
- parse registry LSP entries
- map `pyright`
- if `pyright-langserver` is missing and `--download` is passed, install through `npm`
- make `detect --download playground/python` return a runnable command

Why this is a good first slice:
- simple package manager backend
- common server
- validates end-to-end architecture before harder cases like `jdtls`

Status:
- this milestone is effectively achieved already from the architecture point of view
- the next milestone should focus on real-package coverage and broader verification

Suggested next milestone:
- expand/manual-verify a few more representative Mason-managed servers end-to-end beyond the currently fixed npm cases
- prove at least:
  - `pyright`
  - `typescript-language-server`
  - `rust-analyzer`


## Main Difficult / Risky Parts

- choosing a clean writable runtime-state location
- supporting wrappers correctly for `python`, `dotnet`, `exec`, and `npm`
- handling packages like `jdtls` with shared runtime files
- supporting enough Mason templates without overengineering
- clear fallback behavior when registry refresh fails

Known strategic limitation:
- phase 1 will not support all Mason package ecosystems
- this is acceptable if unsupported cases fail clearly and predictably


## Alternatives Considered

### 1. Vendor a reduced registry snapshot

Pros:
- deterministic
- no network dependency

Cons:
- explicitly rejected
- drifts from Mason over time

### 2. Clone Mason registry git repo

Pros:
- full source fidelity

Cons:
- more complexity
- unnecessary after switching to `registry.json`

### 3. Parse package YAMLs from GitHub directly

Pros:
- close to source package definitions

Cons:
- more work than using compiled `registry.json`

Chosen approach:
- cached GitHub release `registry.json.zip`


## Architectural Consequences

- `lsp-cli` gains a small package-management subsystem.
- Runtime state must be separated from config data.
- The executable resolver should eventually be reused by `run` and query commands.
- If this grows, install resolution may deserve its own higher-level abstraction.

Potential future problem:
- if phase 1 logic is written too narrowly inside `detect`, reuse later will be awkward

Recommended mitigation:
- keep install/executable resolution logic independent from detect rendering


## Start Here In Next Session

1. Decide whether selected additional aliases should be added to the explicit override table.
2. Decide whether exact-only fallback should stay the boundary, or if a slightly broader fallback is needed.
3. Add broader regression tests for registry refresh/install behavior.
4. Manually verify in `playground/` starting with:
   - `playground/python`
   - `playground/typescript`
   - `playground/rust`
   - then extend to `playground/go`, `playground/c` or `playground/cpp`, and `playground/java` where relevant


## Non-goals For The First Pass

- full Mason parity
- non-LSP tools
- every rare package backend
- full schema-download support
- wiring managed installs into every command immediately
