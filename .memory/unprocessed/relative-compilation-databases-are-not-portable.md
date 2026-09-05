# Relative compilation database directories are not portable

The new clangd playgrounds used `"directory": "."` in `compile_commands.json` to avoid embedding
an absolute checkout path. clangd 22.1.6 did not apply those commands after the project was copied
into an isolated E2E workspace, leading to missing include paths and language flags. Compilation
database `directory` fields are required to be absolute, so a tracked portable fixture should use
`compile_flags.txt` or generate its compilation database inside the copied workspace instead.
