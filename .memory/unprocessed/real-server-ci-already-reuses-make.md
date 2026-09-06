# Real-server CI already reuses Make

The pull-request workflow already invokes `make test-real-server-e2e`. Extending that existing
target makes new real-server lifecycle coverage run in CI without copying Cargo commands into
`.github/workflows/ci.yml` or prematurely implementing the next checklist item.
