# LSP config ID differs from CLI server name

The E2E manifest identifies an LSP configuration by the YAML filename stem, such as
`rust_analyzer`. The `--lsp` CLI option does not accept that ID; it selects the configured
user-visible `name`, such as `rust-analyzer`. Generic E2E code must load the name from the selected
LSP YAML instead of passing the manifest ID or duplicating a server-specific name in test code.
