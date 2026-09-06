# Capability matrix revealed transitive runtime prerequisites

The isolated preferred-server matrix showed that Mason's jdtls installation requires `python3`
in addition to Java, and rust-analyzer formatting requires `rustfmt` in addition to Cargo and
rustc. These programs must be staged explicitly because the E2E harness clears ambient `PATH`.
