# Dedicated E2E job moves the existing step

The real-server Make target was already called by the stable entry of the Rust-version matrix.
Adding the planned dedicated job must move that step rather than duplicate it, preserving one E2E
run per push or pull request while exposing an independent CI status.
