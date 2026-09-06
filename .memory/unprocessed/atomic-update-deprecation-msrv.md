# Atomic update rename conflicts with Rust 1.91 MSRV

Rust beta deprecates `AtomicUsize::fetch_update` in favor of `try_update`, but `try_update` was only
stabilized in Rust 1.95 while lsp-cli supports Rust 1.91. A direct rename passes the current compiler
but fails Clippy's `incompatible_msrv` lint. The old spelling needs a narrowly scoped deprecation
allowance until the project raises its MSRV to at least 1.95.
