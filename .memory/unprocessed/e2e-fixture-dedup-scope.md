# E2E fixture deduplication applies to the Rust harness

The project-phase deduplication item applies to repeated E2E harness setup, including validated
manifest loading, isolated context construction, and command-strategy selection. It does not mean
removing the intentionally equivalent domain concepts from the per-language playground sources.
