# Research: which new subcommands are worth adding to lsp-cli

Scope: inventory of currently implemented subcommands and the LSP requests behind
them, the full surface of LSP 3.17 requests that lsp-cli does *not* yet expose, and
how that gap lines up with how AI coding agents actually use LSP servers in
practice (MCP-style code-navigation tool servers such as Serena/multilspy,
Sourcegraph Cody's code-intel layer, and agent "code search" tool sets built into
IDE agent modes). Conclusion is a ranked recommendation, extending the existing
`TODO.md` "commands" wishlist with reasoning.

## 1. Current subcommands and the LSP requests behind them

| Subcommand | LSP request(s) | Category |
|---|---|---|
| `detect` | none (static config) | project setup |
| `languages`, `servers` | none (static config) | project setup |
| `server-capabilities` | `initialize` only | introspection |
| `grep` | `workspace/symbol` | symbol search |
| `list-symbols` | `textDocument/documentSymbol` | symbol search |
| `list-functions` | `textDocument/documentSymbol` (filtered) | symbol search |
| `list-files` | none (fs walk + language detection) | project setup |
| `definition` | `textDocument/definition` | navigation |
| `declaration` | `textDocument/declaration` | navigation |
| `references` | `textDocument/references` | navigation |
| `callers` | `callHierarchy/prepare` + `callHierarchy/incomingCalls` | navigation |
| `callees` | `callHierarchy/prepare` + `callHierarchy/outgoingCalls` | navigation |
| `diagnostics` | `textDocument/diagnostic` (pull) + `textDocument/publishDiagnostics` (push, latest-wins) | verification |
| `format` | `textDocument/formatting` | editing |
| `build-index` | `experimental/serverStatus` / `$/progress` (best-effort, per-server) | project setup |
| `daemon`/`stop`/`stop-all` | `initialize`/`shutdown` proxying, `client/registerCapability`, `workspace/applyEdit` (proxy-only), `workspace/configuration` | infra |
| `run` | dispatches to the above | infra |

This is a coherent "read-only program comprehension" set: find a symbol, see its
shape, see where it's defined/declared, see who references/calls/is called by it,
and verify with diagnostics/formatting. Nothing here mutates code except `format`
(and `workspace/applyEdit`, which today only exists as low-level daemon-proxy
plumbing, not a user-facing command).

`TODO.md` already records a few decisions from a prior pass:
- **Wanted, unimplemented:** type hierarchy, semantic tokens (`??`, undecided).
- **Explicitly rejected:** hover ("too noisy"), moniker ("not usable outside
  LSIF"), code-lens ("not very usable"), inlay-hint ("not useful"),
  executeCommand ("requires edits from the client, mainly for refactoring").
- **Wanted as an option, not a new subcommand:** `-s|--signature` on
  definition-like commands.

## 2. Full LSP request surface not yet used by lsp-cli

Grouped by what they'd add on top of what's already implemented:

**Navigation siblings of definition/declaration/references** (same
location-list result shape, same request/response plumbing already built):
- `textDocument/typeDefinition` — jump from a value to its *type's* declaration.
- `textDocument/implementation` — jump from an interface/abstract member to its
  concrete implementation(s).

**Hierarchy siblings of callers/callees** (same prepare+expand shape already
built for call hierarchy):
- `typeHierarchy/prepare` + `supertypes`/`subtypes` — class/interface hierarchy
  navigation, the OOP analogue of call hierarchy.

**Editing, beyond format:**
- `textDocument/rename` (+ `textDocument/prepareRename`) — protocol-verified,
  cross-file symbol rename via the server's semantic model rather than text
  search-and-replace.
- `textDocument/codeAction` (+ `codeAction/resolve`) — quick fixes / refactors;
  result is either a server-specific `Command` (non-portable) or a
  `WorkspaceEdit` (needs conflict-safe multi-file apply).
- `workspace/executeCommand` — arbitrary server-specific commands; already
  rejected in `TODO.md`.
- `textDocument/rangeFormatting`, `onTypeFormatting` — minor extensions of the
  existing `format`.

**Editor-UI-oriented, no batch/CLI equivalent value:**
- `textDocument/hover` (already rejected), `documentHighlight`,
  `signatureHelp` (wanted only as a flag), `completion` (+
  `completionItem/resolve`), `semanticTokens/*`, `foldingRange`,
  `selectionRange`, `linkedEditingRange`, `documentLink`,
  `documentColor`/`colorPresentation`, `moniker` (already rejected),
  `codeLens` (already rejected), `inlayHint` (already rejected).

**File-operation notifications** (`workspace/willRenameFiles` etc.) — only
relevant if lsp-cli itself renames files on disk, which is out of scope for a
symbol-level tool.

## 3. How AI coding agents actually use LSP servers

Coding agents that wrap an LSP server as an agent tool (MCP-style code-intel
servers, and the code-navigation tool sets built into IDE "agent mode" backends)
converge on a small, stable core:

- **Locate → shape → usage → call-graph.** `workspace/symbol` or
  `documentSymbol` to find a symbol, `definition`/`declaration` to see where
  it's introduced, `references` to see all usages, and call hierarchy
  (incoming/outgoing) to trace control flow. This is exactly lsp-cli's existing
  `grep` / `list-symbols` / `definition` / `declaration` / `references` /
  `callers` / `callees` set — it maps almost one-to-one onto what these agent
  tool servers expose as their primary navigation primitives.
- **Diagnostics as the tight edit-verify loop.** Structured, per-file
  error/warning feedback is one of the highest-value features for an agent,
  because it replaces "run the build/linter" with an immediate, structured
  signal after an edit. lsp-cli's `diagnostics` already covers this.
- **Rename is the most commonly requested *write* capability.** Free-text
  search-and-replace is unsafe for renaming (shadowed locals, string literals,
  comments, cross-module boundaries), while a server-backed rename uses the
  same semantic model as `references`. Of everything not yet implemented, this
  is the one capability agents most concretely lack a safe substitute for.
- **Go-to-implementation / go-to-type-definition close a real navigation gap.**
  In interface-heavy languages (Go, Java, C#, TypeScript, Rust traits),
  "which concrete type implements this interface" and "what type does this
  inferred value actually have" are extremely common follow-up questions that
  `definition`/`declaration` cannot answer, since those resolve to the
  interface/variable itself, not the implementation/type.
- **Hover is usually skipped.** Agent tool servers occasionally expose it, but
  it mostly duplicates markdown doc-comments an agent can already get from
  `documentSymbol`/`definition` context plus its own model knowledge — matching
  lsp-cli's own "too noisy" call.
- **Code actions/quick fixes are rarely exposed as agent tools.** Their results
  are either server-specific commands or workspace edits that conflict with an
  agent that is already editing the repo with its own file tools; agents
  overwhelmingly prefer "make the edit myself, then re-check diagnostics" over
  trusting an opaque, server-specific fix. This matches the existing
  `executeCommand` rejection and extends the same reasoning to `codeAction`.
- **Completion/signatureHelp are essentially never exposed to agents as tools.**
  They're designed for interactive, cursor-position-sensitive, prefix-based
  typing UX; an agent that emits whole edits at once gets little from a raw
  completion list. When signature information is wanted at all, it shows up
  folded into a definition/hover-style lookup, not as a standalone completion
  command — consistent with `TODO.md`'s `--signature` *flag* idea rather than a
  new command.
- **Semantic tokens, folding/selection range, document links, color providers,
  linked editing range: not used by agents at all** in any of these tool
  surfaces — they exist purely to drive editor UI (syntax highlighting,
  code folding, color swatches) with no batch/text equivalent an agent needs.

## 4. Recommendation

**Tier 1 — implement next.** High value, same architectural shape as existing
commands (LSP-generic, no language-specific logic, fits the existing
request-dispatch code in `src/lsp/client/requests.rs`):

1. **`implementation`** (`textDocument/implementation`) — direct sibling of
   `definition`/`declaration`; same location-list result type, same rendering
   path. Fills a real, frequently-hit gap for interface-heavy languages.
2. **`type-definition`** (`textDocument/typeDefinition`) — same rationale, same
   code path as #1.
3. **`rename`** (`textDocument/prepareRename` + `textDocument/rename`) — the
   one genuinely differentiated *write* capability with no safe substitute via
   text tools. Highest value but also highest cost/risk: needs a `WorkspaceEdit`
   applier (the daemon already has a narrower `workspace/applyEdit` proxy path
   in `forwarding.rs` to build on), and should mirror `format`'s `--check`
   /`--stdout`-style dry-run so an agent can preview the edit set before it's
   written to disk.

**Tier 2 — reasonable, lower priority.**

4. **Type hierarchy** (`typeHierarchy/prepare` + `supertypes`/`subtypes`) —
   same prepare-then-expand shape as `callers`/`callees`; useful for OOP
   languages but a narrower audience than #1/#2. Matches the `??` already in
   `TODO.md`.
5. **`--signature` flag** on `definition`/`callers` (surfacing
   `signatureHelp`-equivalent info) rather than a standalone command — as
   `TODO.md` already proposes.

**Tier 3 — do not implement.** Research into agent tool usage confirms the
existing `TODO.md` "NO" list and extends it:

- `hover`, `moniker`, `codeLens`, `inlayHint`, `executeCommand` — already
  rejected; no evidence from agent-facing tool servers that these are used as
  primary tools.
- `codeAction`/`codeAction resolve` — same rejection reasoning as
  `executeCommand`: results are server-specific or require conflict-safe edit
  application that competes with the agent's own editing tools, and can be
  server-language-specific in practice (violates the "no language-specific
  details" principle in `AGENTS.md`).
- `completion`, standalone `signatureHelp`, `semanticTokens/*`,
  `foldingRange`, `selectionRange`, `documentHighlight`, `documentLink`,
  `documentColor`/`colorPresentation`, `linkedEditingRange` — editor-UI
  features with no batch/CLI/agent use observed; existing commands plus an
  agent's own text tools already cover everything these would add.
