# E2E child processes need an isolated TMPDIR

The E2E context securely created its own home, config, runtime, and workspace directories, but did
not set `TMPDIR`. Kotlin LSP consequently created IntelliJ state under `/tmp`. Setting `TMPDIR` on
every harness command is required to keep server-created temporary files inside the secure
`tempfile` sandbox as well.
