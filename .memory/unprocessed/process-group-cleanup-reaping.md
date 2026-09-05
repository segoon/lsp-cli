# Process-group cleanup and descendant reaping

`command-group` successfully sends the kill signal to the E2E command and its descendants, but a
killed grandchild can remain briefly visible in `/proc` as a zombie until its new parent reaps it.
A cleanup regression test must use a bounded wait for the process entry to disappear instead of
interpreting its immediate presence as a live orphan.

After killing and reaping a process group, the RAII guard must discard its `GroupChild` handle.
Leaving the handle armed makes `Drop` attempt a second group kill; in the unlikely event that the
process-group ID has already been reused, that could signal an unrelated process.
