# A parent `/tmp/.git` changes workspace-root tests

Several `suggest` tests create temporary workspaces directly below `/tmp` and configure `.git` as
an LSP root marker. When the host provides `/tmp/.git`, root-marker discovery resolves `/tmp`
instead of the test's temporary workspace, causing otherwise unrelated assertions to fail.

Running the tests with `TMPDIR=/dev/shm` avoids that host-specific parent marker. A durable defense
would make these tests control every ancestor up to the search boundary, or let root discovery stop
at an explicit test boundary, rather than assuming the system temporary directory has no matching
root marker.
