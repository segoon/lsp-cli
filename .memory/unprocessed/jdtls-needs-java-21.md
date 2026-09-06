# Current Mason jdtls requires Java 21

The real-server GitHub job resolved the runner's ambient `java`, but the current Mason jdtls Python
launcher rejected it with `jdtls requires at least Java 21` and exited before the LSP initialize
response. Existence-only host-program resolution does not prove an SDK version is compatible.
