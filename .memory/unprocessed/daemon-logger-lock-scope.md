# Daemon logger isolation does not cover the `run` wrapper before exec

The daemon starts its configured server through a child `lsp-cli run` process. That wrapper writes
startup records through the global synchronous system logger before it replaces itself with the
actual LSP server. Consequently, holding the global log lock before initial daemon startup can still
delay server startup even after daemon-owned logging is isolated. The latency playground must acquire
the lock after the daemon reports `READY` when testing coordinator responsiveness.
