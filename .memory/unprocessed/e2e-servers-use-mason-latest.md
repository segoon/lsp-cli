# Real-server E2E provisioning uses Mason latest

The user corrected the preferred-server E2E design: do not install LSP servers separately and do
not pin a Mason registry release or package versions. Each real-server test must use `--download`
against Mason latest. The preferred smoke pair must be derived from the first production preference
in `data/lsp-cli.yaml`; do not duplicate that selection in the E2E case YAML.

This deliberately trades reproducibility for immediate upstream compatibility coverage. Preserve
the resolved Mason source ID in diagnostics so failures can still identify the installed version.
