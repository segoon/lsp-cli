# Child ownership determines lifecycle cancellation

`std::process::Child` cannot be cloned. A worker that waits and reaps the child must
also receive force-stop commands and own termination. This keeps the daemon
coordinator responsive, but an unexpected daemon unwind can only send the worker a
best-effort stop command; joining that worker would restore an unlimited process
wait on the coordinator.
