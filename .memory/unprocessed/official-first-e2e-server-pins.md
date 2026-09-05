# Official-first E2E server pins expose launcher mismatches

The user selected an official-first preferred E2E server matrix and explicitly accepted bespoke
provisioning and current command/config gaps. In particular, use `kotlin_lsp` rather than the
community `kotlin_language_server`, and `roslyn_ls` rather than OmniSharp.

The pinned Mason registry exposes Kotlin LSP as `intellij-server`, although the data config starts
`kotlin-lsp`. It exposes Roslyn as `roslyn-language-server`, although the data config starts `dotnet`
with a literal `<my_folder>` DLL path. The preferred-server selection can be committed independently,
but provisioning must resolve these mismatches before claiming either smoke case is runnable.
