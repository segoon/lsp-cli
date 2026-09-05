# E2E runtime-root constraints

The E2E daemon runtime directory cannot be nested below the ordinary test root. The production
socket suffix (`lsp-cli/`, a server slug of up to 32 characters, a separator, a 24-character hash,
and `.sock`) makes that layout exceed the Unix-domain socket path limit in the current environment.
Create the runtime `TempDir` directly below the selected secure base with a short prefix.

The configured build-time `XDG_RUNTIME_DIR` may also be mounted read-only by an execution sandbox
even though it is writable in the host environment. Tests using the secure root can therefore need
the test command to run with the sandbox's filesystem restriction lifted; falling back to `/tmp`
would violate the project requirement and conceal the actual path behavior.
