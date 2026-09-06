# Foundation lifecycle test coupled stop-all to same-socket restart

GitHub CI once reported that the second daemon in `lifecycle_command_paths_are_covered` was no
longer active when `stop-all` ran, although 200 focused local iterations passed. The test stopped a
daemon and immediately created another daemon for the identical workspace/server socket before
testing `stop-all`.

The foundation command-coverage test now uses a second isolated workspace, and therefore a distinct
socket, for `stop-all`. Same-socket restart behavior remains covered by the dedicated real-server
lifecycle scenarios. Failed stdout assertions now include captured process diagnostics and runtime
state so a recurrence will reveal whether `stop-all` stopped a daemon or removed a stale socket.
