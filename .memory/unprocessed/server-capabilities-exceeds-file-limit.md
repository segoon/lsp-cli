# Server-capabilities source exceeds the repository file limit

The final line-count audit found `src/commands/server_capabilities.rs` at 651 lines although
`AGENTS.md` limits every file, including tests, to 600 lines. This file was not changed by the
manual E2E survey, so splitting it was left outside the survey commit rather than mixed into
fixture portability and compatibility evidence.
