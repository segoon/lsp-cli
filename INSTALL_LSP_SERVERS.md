# Install LSP Servers

This document describes how `lsp-cli` installs LSP servers automatically from Mason metadata.

## Common Flow

1. `resolve_or_install_program()` parses the Mason source id and picks an `install_*` backend.
2. Each backend first resolves the expected executable path for the selected program.
3. If that executable is already runnable, installation is skipped and the cached path is reused.
4. Otherwise the backend installs into the per-user runtime state under `~/.local/share/lsp-cli/`.
5. `finalize_install()` verifies the resulting executable, materializes `share/` files, and writes a receipt.

## Common Requirements

- `HOME` must be set so `lsp-cli` can locate its runtime state.
- Network access is required for backends that download packages or archives.
- HTTPS downloads use `reqwest` with the `rustls-tls` backend, so server certificates are verified by default.
- Downloaded archives are unpacked with a 512 MiB per-entry decoded-size limit.
- Installed launchers are hardened to avoid group/other write bits on Unix.

## `install_npm_package`

How it works:
- Requires `npm` in `PATH`.
- Runs `npm install --no-package-lock --prefix <install-dir> <package>@<version> ...extra_packages`.
- Expects Mason's `npm:` executable mapping to resolve under `node_modules/.bin/`.

Security implications:
- Executes npm lifecycle behavior provided by upstream packages.
- Trust is delegated to the npm registry, the package publisher, and any transitive packages.

Limitations:
- Depends on npm being configured correctly on the host.
- Extra Mason packages are passed through as-is.

## `install_pypi_package`

How it works:
- Requires `python3` in `PATH` and a working `python3 -m pip`.
- Runs `python3 -m pip install --disable-pip-version-check --prefix <install-dir> <spec>`.
- Supports Mason `extra=` qualifiers by rendering `package[extra]==version`.

Security implications:
- Executes installer code from Python packages and dependencies.
- Trust is delegated to PyPI and the selected package set.

Limitations:
- Uses the host Python environment and its pip behavior.
- Wrapper-based launchers are currently Unix-only.

## `install_cargo_package`

How it works:
- Requires `cargo` in `PATH`.
- Runs `cargo install --root <install-dir> --version <version> <package>`.
- Expects Mason's `cargo:` executable mapping to resolve under `<install-dir>/bin/`.

Security implications:
- Builds and executes Rust build scripts from the selected crate graph.
- Trust is delegated to crates.io or the referenced cargo source.

Limitations:
- Build time and toolchain requirements depend on the package.

## `install_golang_package`

How it works:
- Requires `go` in `PATH`.
- Runs `go install <module>@<version>` with `GOBIN` set to the resolved install directory.
- Expects Mason's `golang:` executable mapping to resolve under that bin directory.

Security implications:
- Downloads and builds Go modules from the configured module ecosystem.

Limitations:
- Behavior depends on the host Go toolchain and module settings.

## `install_github_package`

How it works:
- Selects the Mason asset for the detected platform.
- Downloads the GitHub release asset over HTTPS.
- Unpacks `.tar.gz`, `.zip`, and `.gz` payloads, or writes plain files directly.
- Resolves the final executable path from the rendered Mason template context.

Security implications:
- Trust is delegated to the GitHub release asset publisher.
- Archive paths are checked to stay within the target install root.

Limitations:
- Platform support is limited to targets described in the Mason package metadata.
- Wrapper launchers generated for `python:`, `node:`, `dotnet:`, and `java-jar:` are Unix-only today.

## `install_generic_package`

How it works:
- Selects Mason `download:` entries for the detected platform.
- Downloads each declared file over HTTPS and materializes it into the package directory.
- Resolves binaries and `share/` paths from the rendered download template context.

Security implications:
- Trust is delegated to the upstream URLs embedded in Mason metadata.

Limitations:
- Behavior depends entirely on the correctness of Mason's declarative download metadata.

## Work Modes

- Cached mode: if the resolved executable is already runnable, installation is skipped.
- Install mode: missing executables are fetched or built on demand.
- Resolve-only mode: unsupported Mason backends return a clear user-facing error instead of guessing.

## Future Concerns

- Full Windows wrapper generation is still missing.
- A future typed-error migration may help internal structure, but it should be done consistently across command/config/install layers instead of piecemeal.
