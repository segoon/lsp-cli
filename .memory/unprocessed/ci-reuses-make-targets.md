# CI reuses Makefile verification targets

The initial E2E implementation duplicated formatting, Clippy, and live-server commands between
`Makefile` and `.github/workflows/ci.yml`. The user explicitly rejected that structure. Keep the
commands centralized as granular Make targets; CI should only choose which Make target runs for a
toolchain or lane.
