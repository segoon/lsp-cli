# Real-server E2E cases are manifest-driven

The first proposed real-server proof was shaped as a Rust/rust-analyzer-specific test. The user
corrected that direction: the harness must extend to any LSP server and must not hardcode one
server. Keep language IDs, server config IDs, expected symbols, and runtime prerequisites in the
E2E manifest. Runner code may dispatch only on generic operation and provisioning types.
