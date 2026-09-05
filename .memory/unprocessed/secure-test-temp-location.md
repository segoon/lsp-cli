# Real test sandboxes must not use ambient `/tmp`

The user clarified that real test state must not be created under `/tmp`, even through the secure
default `tempfile::Builder::tempdir` API. Unit and E2E test helpers should use the existing
`tempfile` crate to create randomized exclusive directories under `XDG_RUNTIME_DIR`,
`XDG_CACHE_HOME`, or `$HOME/.cache`, in that order.

Synthetic `/tmp` strings used only to test URI or path parsing do not create files and are outside
this policy.
