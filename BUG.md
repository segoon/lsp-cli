# Initialize workspaceFolders mismatch

## Summary

`lsp-cli` sent `workspaceFolders` in the `initialize` request, but it didn't advertise
`workspace.workspaceFolders: true` and later answered `workspace/workspaceFolders` with `[]`.
That made the handshake internally inconsistent for workspace-aware servers.

## Why this was a bug

- LSP 3.17 says `InitializeParams.workspaceFolders` is only available when the client supports
  workspace folders.
- LSP 3.17 also defines `workspace/workspaceFolders` as the way for the server to query the same
  current folder list later.
- Before the fix, lsp-cli provided one folder during `initialize`, but returned an empty list when
  asked again.

## Affected code

- Request construction: `src/lsp/client/requests.rs`
- Client request handling: `src/lsp/client.rs`

## Impact

- Workspace-aware servers could downgrade themselves or make incorrect assumptions about the active
  project roots.
- The behavior violated the protocol contract even though many servers likely tolerated it.

## Fix

- Advertise `workspace.workspaceFolders: true` in `initialize`.
- Keep the current workspace folder list in the client state.
- Reply to `workspace/workspaceFolders` with that same list.
- Added regression coverage in `src/lsp/client/tests.rs`.

## Tradeoffs

- Pros: protocol-correct, consistent, low-risk, matches the existing one-root workspace model.
- Cons: adds a small piece of client state to preserve the advertised folder list.

## Alternatives considered

- Stop sending `workspaceFolders` at all.
  - Pros: smaller protocol surface.
  - Cons: throws away useful workspace-root information and is less future-proof.
- Advertise the capability but keep returning `[]`.
  - Pros: none.
  - Cons: still incorrect.
