# Encountered difficulties

## What confused me

- The environment contains `/tmp/.git`. Three existing workspace-root tests then
  discover `/tmp` as their project root. Running `TMPDIR=/dev/shm make test` isolates
  fixture ancestry and passes those tests without changing unrelated code.
- Tests change PATH while other tests run. The new daemon echo-helper test must use
  `/bin/cat`, not a PATH lookup. This avoids introducing another environment race.
- Unix socket creation is blocked in the sandbox; socket regression tests needed
  execution outside it.
- The rust-analyzer command is a rustup proxy with no installed component. LuaLS was
  available, so a small Lua playground was added for real-server validation.
- One isolated benchmark daemon returned an unexpected stop-response ID. Retrying
  cleanup removed its stale socket; this is recorded in GOTCHAS.md and remains
  separate from request-window scheduling.

# Missing tools

`make test` reaches `cargo deny check`, but cargo-deny is not installed. Formatting,
270 active tests (one ignored subprocess helper), Clippy, and README generation
consistency checks pass. No new dependency or tool was installed.

## Where to report

If you're sure the reported difficulties above are related to techplatform (e.g. userver, c35),
please report to [aisuite](https://nda.ya.ru/t/EcUMOwSH7eudWX).
