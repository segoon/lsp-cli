# Current Mason kotlin-lsp build expires before initialization

After adding generic support for `version | strip_prefix "<literal>"`, Mason successfully downloaded
and launched `kotlin-lsp/v262.9593.0`. The packaged `intellij-server` then reported that its build
had expired and exited before completing LSP initialization. This replaced the earlier assumption
that template rendering was the only blocker to the Kotlin survey.
