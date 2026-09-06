# Real-server suites must not compete inside one test binary

Broadening the Make target's libtest filter caused the query and lifecycle matrix tests to run in
parallel. Concurrent clean-home downloads produced a GitHub transport failure, and CPU/server load
made an Objective-C++ query case consume its existing overall deadline. Both cases passed alone.
The two top-level real-server tests should run with one libtest thread; each still manages its own
sequential matrix and isolated state.
