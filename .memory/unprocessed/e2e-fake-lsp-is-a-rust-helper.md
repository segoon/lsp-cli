# The deterministic E2E LSP fixture is a Rust helper

When offered Python, real rust-analyzer, or a Rust helper for deterministic command-path tests,
the user selected the Rust helper. Keep the helper as an isolated, locked fixture package that
reuses serde_json; do not replace it with a Python script merely to reduce fixture build code.
