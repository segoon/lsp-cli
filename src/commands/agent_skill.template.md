---
name: lsp-cli
description: Use lsp-cli for semantic code navigation, diagnostics, and formatting from the terminal.
---

# lsp-cli skill

This skill helps a code agent use `lsp-cli` for semantic code navigation, diagnostics, and formatting from the terminal without editor-specific LSP integration.

## When to use lsp-cli
- Use it when you need semantic workspace navigation instead of plain text search.
- Use it when the agent runs in a shell, container, CI job, or SSH session without editor-managed LSP integration.
- Use it when the repository uses a rare or proprietary language server that the agent does not know how to configure directly.
- Use it when you need LSP diagnostics or formatting as part of an edit/verify loop.

## Rules of thumb
- Prefer `--json` when the output will be parsed or summarized by the agent.
- Prefer `--limit <N>` to avoid flooding the agent context with large workspaces.
- Always use `--detach` to avoid LSP server start/stop on each invocation.
- Fall back to plain file/content search when an LSP feature is unsupported or the result is obviously incomplete.

## Core commands

All commands in this section support these flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}

### `grep`

```sh
# {CMD/GREP}
# Use it when you need semantic workspace symbol search before opening or editing files.
# Matching behavior heavily depends on the LSP server.
lsp-cli grep --detach --json --limit 20 Order path/to/project
```
Useful flags:
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `list-symbols`
```sh
# {CMD/LIST_SYMBOLS}
# Use it when you need a symbol outline for one file or a workspace slice.
# Pass a file path for a focused outline or a directory for a broader scan.
lsp-cli list-symbols --detach --json --limit 50 path/to/project/src/main.rs
```

### `list-functions`
```sh
# {CMD/LIST_FUNCTIONS}
# Use it when you want a compact list of callable entry points in a workspace.
# Useful for discovering candidate APIs before deeper navigation.
lsp-cli list-functions --detach --json --limit 50 path/to/project
```

### `list-files`
```sh
# {CMD/LIST_FILES}
# Use it when you need the file set that the selected LSP workspace query will consider.
# Useful before diagnostics or workspace-wide symbol queries in mixed repositories.
lsp-cli list-files --detach --json --limit 100 path/to/project
```

### `definition`
```sh
# {CMD/DEFINITION}
# Use it when you need the implementation location for a symbol before editing or reading code.
# Use `--full` only when you need the returned source snippet, because it can expand output a lot.
lsp-cli definition --detach --json --limit 10 MySymbol path/to/project
```
Useful flags:
- `--full`: {OPT/FULL}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `declaration`
```sh
# {CMD/DECLARATION}
# Use it when you need the declared API location rather than the implementation site.
# This is most useful in languages that distinguish declarations from definitions.
lsp-cli declaration --detach --json --limit 10 MySymbol path/to/project
```
Useful flags:
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `references`
```sh
# {CMD/REFERENCES}
# Use it when you need impact analysis before a rename, signature change, or behavior change.
# Prefer this before wide edits so the agent does not miss indirect usage sites.
lsp-cli references --detach --json --limit 100 MySymbol path/to/project
```
Useful flags:
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `callers`
```sh
# {CMD/CALLERS}
# Use together with `callees` to sketch a local call graph.
lsp-cli callers --detach --json --limit 50 format_order path/to/project
```
Useful flags:
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `callees`
```sh
# {CMD/CALLEES}
# Use it when you need to understand which functions a symbol depends on.
# This is useful for estimating side effects before touching a function body.
lsp-cli callees --detach --json --limit 50 format_order path/to/project
```
Useful flags:
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `diagnostics`
```sh
# {CMD/DIAGNOSTICS}
# Use it when you need LSP-reported errors and warnings after making edits.
# Use this after edits even when tests pass, because the language server may report unresolved symbols or type issues.
lsp-cli diagnostics --detach --json --limit 100 path/to/project
```
Useful flags:
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `format`
```sh
# {CMD/FORMAT}
lsp-cli format path/to/file.rs
```
Useful flags:
- `--check`: {OPT/CHECK}
- `--stdout`: {OPT/STDOUT}

## Setup and troubleshooting

If automatic selection is ambiguous, these options help:
- `--lang <LANG>`: {OPT/LANG}
- `--lsp <LSP>`: {OPT/LSP}

### `servers`
```sh
# {CMD/SERVERS}
# Use it when you need to discover valid `--lsp` names, especially after narrowing to one language.
lsp-cli servers --lang python
```
Useful flags:
- `--lang <LANG>`: List servers configured for this language only.

## Limitations
- Results are only as good as the selected LSP server.
- Not every server supports every feature.
- `workspace/symbol` quality and pattern syntax vary between servers.
- Background indexing support varies, so `--wait-for-index` may help on some servers and do nothing on others.
