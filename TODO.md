detect:
- multiple LSP servers for the same filetype: priority
- output file count

commands:
- symbol|search|find
- symbol-definition
- symbol-declaration
- references|refs
- callers
- callees
- symbols-file
- symbols-workspace
- repl (TODO: name... cli, console, terminal, interactive?)

lifecycle:
- start+stop - in background
- status
- addr - show active LSP server
- serve - synch start LSP server, stop it on exit (TODO: what to do with multiple LSP servers?)
