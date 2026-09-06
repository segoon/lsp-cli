# jdtls stop socket disappearance precedes process exit

The detached lifecycle E2E test found that `lsp-cli stop` can return after unlinking the daemon
socket while the upstream jdtls Java process is still shutting down. Restarting the same workspace
at that point overlaps jdtls instances and can stall the new client. A bounded process-exit check
is needed between exact stop and restart; socket absence alone is insufficient lifecycle evidence.
