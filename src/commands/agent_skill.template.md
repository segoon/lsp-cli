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
- Always use --detach to avoid LSP server start/stop on each invocation.
- Fall back to plain file/content search when an LSP feature is unsupported or the result is obviously incomplete.

## Core commands

### `grep`
Purpose: {CMD/GREP}
Use it when: you need semantic workspace symbol search before opening or editing files.
Note: This uses `workspace/symbol`, so matching behavior depends on the server.
Example:
```sh
lsp-cli grep --json --limit 20 Order path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `list-symbols`
Purpose: {CMD/LIST_SYMBOLS}
Use it when: you need a symbol outline for one file or a workspace slice.
Note: Pass a file path for a focused outline or a directory for a broader scan.
Example:
```sh
lsp-cli list-symbols --json --limit 50 path/to/project/src/main.rs
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}

### `list-functions`
Purpose: {CMD/LIST_FUNCTIONS}
Use it when: you want a compact list of callable entry points in a workspace.
Note: Useful for discovering candidate APIs before deeper navigation.
Example:
```sh
lsp-cli list-functions --json --limit 50 path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}

### `list-files`
Purpose: {CMD/LIST_FILES}
Use it when: you need the file set that the selected LSP workspace query will consider.
Note: Useful before diagnostics or workspace-wide symbol queries in mixed repositories.
Example:
```sh
lsp-cli list-files --json --limit 100 path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}

### `definition`
Purpose: {CMD/DEFINITION}
Use it when: you need the implementation location for a symbol before editing or reading code.
Note: Use `--full` only when you need the returned source snippet, because it can expand output a lot.
Example:
```sh
lsp-cli definition --json --limit 10 MySymbol path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `--full`: {OPT/FULL}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `declaration`
Purpose: {CMD/DECLARATION}
Use it when: you need the declared API location rather than the implementation site.
Note: This is most useful in languages that distinguish declarations from definitions.
Example:
```sh
lsp-cli declaration --json --limit 10 MySymbol path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `--full`: {OPT/FULL}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `references`
Purpose: {CMD/REFERENCES}
Use it when: you need impact analysis before a rename, signature change, or behavior change.
Note: Prefer this before wide edits so the agent does not miss indirect usage sites.
Example:
```sh
lsp-cli references --json --limit 100 MySymbol path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `callers`
Purpose: {CMD/CALLERS}
Use it when: you need to understand which code paths invoke a function.
Note: Use together with `callees` to sketch a local call graph.
Example:
```sh
lsp-cli callers --json --limit 50 format_order path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `callees`
Purpose: {CMD/CALLEES}
Use it when: you need to understand which functions a symbol depends on.
Note: This is useful for estimating side effects before touching a function body.
Example:
```sh
lsp-cli callees --json --limit 50 format_order path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `diagnostics`
Purpose: {CMD/DIAGNOSTICS}
Use it when: you need LSP-reported errors and warnings after making edits.
Note: Use this after edits even when tests pass, because the language server may report unresolved symbols or type issues.
Example:
```sh
lsp-cli diagnostics --json --limit 100 path/to/project
```
Recommended flags:
- `--json`: {OPT/JSON}
- `--limit <N>`: {OPT/LIMIT}
- `--wait-for-index`: {OPT/WAIT_FOR_INDEX}
- `--detach`: {OPT/DETACH}
- `-l, --files-with-matches`: {OPT/FILES_WITH_MATCHES}

### `format`
Purpose: {CMD/FORMAT}
Use it when: you need language-server-native formatting or a formatting check before finishing an edit.
Note: Use `--stdout` when the agent wants to inspect formatting changes before rewriting the file.
Example:
```sh
lsp-cli format --check path/to/file.rs
```
Recommended flags:
- `--check`: {OPT/CHECK}
- `--stdout`: {OPT/STDOUT}
- `--json`: {OPT/JSON}
- `--detach`: {OPT/DETACH}

## Setup and troubleshooting

If automatic selection is ambiguous, these options help:
- `--lang <LANG>`: {OPT/LANG}
- `--lsp <LSP>`: {OPT/LSP}

### `languages`
Purpose: {CMD/LANGUAGES}
Use it when: you need to discover the canonical language ids accepted by `--lang`.
Note: This is mainly useful when `detect` reports multiple languages or a guess needs to be forced manually.
Example:
```sh
lsp-cli languages
```

### `servers`
Purpose: {CMD/SERVERS}
Use it when: you need to discover valid `--lsp` names, especially after narrowing to one language.
Note: Use this to pick a different configured server when the default one behaves poorly.
Example:
```sh
lsp-cli servers --lang python
```
Recommended flags:
- `--lang <LANG>`: List servers configured for this language only.

## Limitations
- Results are only as good as the selected LSP server.
- Not every server supports every feature.
- `workspace/symbol` quality and pattern syntax vary between servers.
- Background indexing support varies, so `--wait-for-index` may help on some servers and do nothing on others.
