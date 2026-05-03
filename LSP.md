# Initialize requests and responses in lsp-cli

## Specification

- Canonical spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/>
- Primary sections:
  - `#initialize`
  - `#initialized`
  - `#clientCapabilities`
  - `#serverCapabilities`
  - `#workspace_workspaceFolders`
  - per-feature sections such as `#textDocument_definition`, `#textDocument_declaration`,
    `#textDocument_documentSymbol`, and `#workspace_symbol`

## What initialize means

- `initialize` is the first client request.
- The server answers with `InitializeResult`.
- The client then sends `initialized` exactly once.
- Client and server capabilities are negotiated during this handshake.

## Where lsp-cli handles it

- Client request construction and response decoding: `src/lsp/client/requests.rs`
- Generic client request handling: `src/lsp/client.rs`
- Capability gating used by commands: `src/lsp/capabilities.rs`
- Command entry point that initializes an LSP session before queries: `src/commands/symbol_query.rs`
- Background-work waiting built on initialize-time capabilities: `src/lsp/client/background.rs`
- Daemon normalize/cache/reuse logic:
  - `src/commands/daemon.rs`
  - `src/commands/daemon/protocol.rs`
  - `src/commands/daemon/process.rs`

## What depends on initialize in lsp-cli

- `workspace-symbol`
- `list-functions`
- `list-symbols`
- `references`
- `definition`
- `declaration`
- `callers`
- `callees`
- `build-index`
- daemon upstream reuse, since the daemon fingerprints normalized `initialize` params and caches
  the `initialize` result

## What lsp-cli currently sends in initialize

- `processId`
- `rootUri`
- `workspaceFolders`
- `clientInfo`
- `window.workDoneProgress`
- `general.positionEncodings = ["utf-16"]`
- experimental `serverStatusNotification`

## What lsp-cli currently uses from InitializeResult

Only a narrow subset of `ServerCapabilities` is decoded and checked:

- `workspaceSymbolProvider`
- `documentSymbolProvider`
- `referencesProvider`
- `definitionProvider`
- `declarationProvider`
- `callHierarchyProvider`

Code: `src/lsp/capabilities.rs`

## Existing initialize-dependent behavior

- `workspaceSymbolProvider` gates project-wide symbol lookup.
- `documentSymbolProvider` gates `list-functions`, `list-symbols`, and the document-level fallback
  used to refine workspace-symbol results.
- `referencesProvider`, `definitionProvider`, `declarationProvider`, and
  `callHierarchyProvider` gate their respective query commands.
- `window.workDoneProgress` and experimental `serverStatusNotification` are used for
  `wait-for-index` / `build-index` flows.

## Significant capabilities and fields lsp-cli may want

### Already important

- `workspaceSymbolProvider`
- `documentSymbolProvider`
- `referencesProvider`
- `definitionProvider`
- `declarationProvider`
- `callHierarchyProvider`
- `window.workDoneProgress`
- experimental `serverStatusNotification`

### Valuable additions

- `textDocument.definition.linkSupport`
  - lsp-cli already parses `LocationLink` results.
  - Pros: better target precision from servers that can return links.
  - Cons: slightly larger advertised surface.
- `textDocument.declaration.linkSupport`
  - Same reasoning as definition.
- `textDocument.documentSymbol.hierarchicalDocumentSymbolSupport`
  - lsp-cli already handles nested `DocumentSymbol` trees.
  - Pros: may improve accuracy and allow richer server responses.
  - Cons: no major downside beyond broader client claims.
- `workspace.symbol.symbolKind.valueSet`
- `textDocument.documentSymbol.symbolKind.valueSet`
  - Pros: clearer support for newer symbol kinds.
  - Cons: extra initialization detail with little immediate payoff unless servers use it.
- `InitializeResult.serverInfo`
  - Currently ignored.
  - Pros: useful for debugging and user-facing diagnostics.
  - Cons: mostly informational.
- `workspace.symbol.resolveSupport`
  - Pros: future support for partial workspace symbols and lazy resolution.
  - Cons: requires extra request flow and result plumbing.

## Important fields lsp-cli currently ignores

- `InitializeResult.serverInfo`
- most of `ServerCapabilities`
- negotiated `capabilities.positionEncoding`

## Position encoding notes

- lsp-cli advertises only UTF-16.
- That is safe today because all offsets it sends are therefore UTF-16 by construction.
- If lsp-cli ever advertises more than UTF-16, it will need to honor the server-selected
  `capabilities.positionEncoding` from `InitializeResult` when building request positions.

## Dynamic registration notes

- LSP allows servers to register capabilities dynamically after `initialized`.
- lsp-cli already answers `client/registerCapability` and `client/unregisterCapability`.
- The daemon treats dynamic registrations conservatively and restarts before reuse when needed.

## Bug found during review

- Before the fix, lsp-cli sent `workspaceFolders` in `initialize`, but did not advertise
  `workspace.workspaceFolders` and later returned `[]` for `workspace/workspaceFolders`.
- This was a protocol mismatch and could confuse workspace-aware servers.
- Details are written in `BUG.md`.

## Fix applied

- Added `workspace.workspaceFolders: true` to the `initialize` client capabilities.
- Stored the workspace folder list in the client state.
- Replied to `workspace/workspaceFolders` with the same folder list sent in `initialize`.
- Added a regression test covering both the advertised capability and the later response.

## Architectural consequences

- The fix keeps lsp-cli aligned with its existing one-root workspace model.
- It does not add multi-root support.
- If lsp-cli later adds true multi-root workspaces, this state and response path are the natural
  place to extend.

## Alternatives considered

- Remove `workspaceFolders` from `initialize`.
  - Pros: less protocol surface.
  - Cons: loses useful workspace semantics and avoids, rather than fixes, the mismatch.
- Keep current behavior and rely on tolerant servers.
  - Pros: zero code changes.
  - Cons: protocol bug remains and can break stricter servers.
