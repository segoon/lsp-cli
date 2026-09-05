# Test temp roots must leave room for Unix socket names

Creating secure test sandboxes below this worktree's `target/test-tmp/` made the absolute paths long
enough that existing Unix listener tests failed with `path must be shorter than SUN_LEN`. Workspace
and daemon test helpers must select a short per-user runtime or cache root, not merely any secure
repository-local directory.

This should be defended by keeping test directory prefixes compact and by retaining socket tests
that bind the longest production-shaped daemon path.

The root must also be selected from the build-time environment. Reading `XDG_RUNTIME_DIR` or
`HOME` while tests run races with existing tests that temporarily replace process-wide environment
variables and can place one test's sandbox inside another test's short-lived directory.
