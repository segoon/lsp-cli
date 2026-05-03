# Server capabilities

## `positionEncoding`

It means: the server chooses the position encoding it wants to use for LSP positions.
It can be used by lsp-cli this way: honor the negotiated encoding when turning source offsets into LSP positions; today lsp-cli only advertises UTF-16, so this is mostly future-facing.

## `textDocumentSync`

It means: the server describes how it wants text documents to be synchronized.
It can be used by lsp-cli this way: decide whether to send open, change, save, and close notifications and in what shape.

- `textDocumentSync.openClose`: server wants open/close notifications. lsp-cli can use it to decide whether `didOpen` and `didClose` are necessary.
- `textDocumentSync.change`: server wants change notifications and declares the sync kind. lsp-cli can use it if it starts sending incremental or full document updates.
- `textDocumentSync.willSave`: server wants will-save notifications. lsp-cli can use it before file writes if it ever edits documents through LSP workflows.
- `textDocumentSync.willSaveWaitUntil`: server wants will-save-wait-until requests. lsp-cli can use it to fetch pre-save edits before saving.
- `textDocumentSync.save`: server wants save notifications. lsp-cli can use it after writing files.
- `textDocumentSync.save.includeText`: server expects document text to be included on save. lsp-cli can use it to decide whether `didSave` should carry content.

## `notebookDocumentSync`

It means: the server describes how it wants notebook documents and notebook cells synchronized.
It can be used by lsp-cli this way: only if lsp-cli grows notebook support; today it is not used.

- `notebookDocumentSync.notebookSelector`: selectors that describe which notebooks the server cares about. lsp-cli can use them to match supported notebook types.
- `notebookDocumentSync.notebookSelector.notebook`: notebook filter entry. lsp-cli can use it when deciding whether a notebook is in scope.
- `notebookDocumentSync.notebookSelector.notebook.notebookType`: notebook type filter. lsp-cli can use it to route only matching notebook kinds.
- `notebookDocumentSync.notebookSelector.notebook.scheme`: URI scheme filter. lsp-cli can use it to restrict notebook sync to `file`, `untitled`, and similar schemes.
- `notebookDocumentSync.notebookSelector.notebook.pattern`: glob filter for notebooks. lsp-cli can use it to match notebook paths.
- `notebookDocumentSync.notebookSelector.cells`: cell selectors. lsp-cli can use them to decide which cells participate in sync.
- `notebookDocumentSync.notebookSelector.cells.language`: cell language filter. lsp-cli can use it to sync only relevant cell languages.
- `notebookDocumentSync.save`: server wants notebook save notifications. lsp-cli can use it if notebook save flows are implemented.
- `notebookDocumentSync.id`: registration id for notebook sync. lsp-cli can use it when tracking or unregistering dynamic registrations.

## `completionProvider`

It means: the server supports completion and declares how completion should be triggered and resolved.
It can be used by lsp-cli this way: implement a future `completion` command with correct trigger and resolve behavior.

- `completionProvider.workDoneProgress`: completion requests may report progress. lsp-cli can surface it in long-running completion flows.
- `completionProvider.triggerCharacters`: characters that trigger completion. lsp-cli can use them for editor-like or scripted trigger behavior.
- `completionProvider.allCommitCharacters`: global commit characters. lsp-cli can use them if it ever applies a selected completion item.
- `completionProvider.resolveProvider`: completion items can be lazily resolved. lsp-cli can call `completionItem/resolve` only when needed.
- `completionProvider.completionItem`: server-specific completion item capability container. lsp-cli can inspect nested flags when adding completion support.
- `completionProvider.completionItem.labelDetailsSupport`: server can send label details. lsp-cli can render richer completion labels.

## `hoverProvider`

It means: the server supports hover.
It can be used by lsp-cli this way: implement a future `hover` command.

- `hoverProvider.workDoneProgress`: hover requests may report progress. lsp-cli can surface it for slow servers.

## `signatureHelpProvider`

It means: the server supports signature help.
It can be used by lsp-cli this way: implement a future `signature-help` command.

- `signatureHelpProvider.workDoneProgress`: signature-help requests may report progress. lsp-cli can surface it if useful.
- `signatureHelpProvider.triggerCharacters`: characters that trigger signature help. lsp-cli can use them in editor-like integrations.
- `signatureHelpProvider.retriggerCharacters`: characters that retrigger signature help. lsp-cli can use them when simulating interactive typing.

## `declarationProvider`

It means: the server supports go-to-declaration.
It can be used by lsp-cli this way: this is already used to gate the `declaration` command.

- `declarationProvider.workDoneProgress`: declaration requests may report progress. lsp-cli can surface it if declaration queries become long-running.
- `declarationProvider.documentSelector`: declaration support is scoped to certain documents. lsp-cli can use it to avoid unsupported files.
- `declarationProvider.id`: registration id for declaration support. lsp-cli can track it if it deepens dynamic-registration support.

## `definitionProvider`

It means: the server supports go-to-definition.
It can be used by lsp-cli this way: this is already used to gate the `definition` command.

- `definitionProvider.workDoneProgress`: definition requests may report progress. lsp-cli can surface it if useful.

## `typeDefinitionProvider`

It means: the server supports go-to-type-definition.
It can be used by lsp-cli this way: implement a future `type-definition` command.

- `typeDefinitionProvider.workDoneProgress`: type-definition requests may report progress. lsp-cli can surface it.
- `typeDefinitionProvider.documentSelector`: support is scoped to certain documents. lsp-cli can use it to avoid unsupported files.
- `typeDefinitionProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `implementationProvider`

It means: the server supports go-to-implementation.
It can be used by lsp-cli this way: implement a future `implementation` command.

- `implementationProvider.workDoneProgress`: implementation requests may report progress. lsp-cli can surface it.
- `implementationProvider.documentSelector`: support is scoped to certain documents. lsp-cli can use it to avoid unsupported files.
- `implementationProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `referencesProvider`

It means: the server supports find-references.
It can be used by lsp-cli this way: this is already used to gate the `references` command.

- `referencesProvider.workDoneProgress`: references requests may report progress. lsp-cli can surface it for large workspaces.

## `documentHighlightProvider`

It means: the server supports document highlights.
It can be used by lsp-cli this way: implement a future `document-highlight` command.

- `documentHighlightProvider.workDoneProgress`: document-highlight requests may report progress. lsp-cli can surface it if needed.

## `documentSymbolProvider`

It means: the server supports document symbols.
It can be used by lsp-cli this way: this is already used for `list-functions`, `list-symbols`, and workspace-symbol refinement.

- `documentSymbolProvider.workDoneProgress`: document-symbol requests may report progress. lsp-cli can surface it when scanning many files.
- `documentSymbolProvider.label`: user-facing label for multiple outline providers. lsp-cli can render it if it ever exposes provider identity.

## `codeActionProvider`

It means: the server supports code actions.
It can be used by lsp-cli this way: implement a future `code-action` command.

- `codeActionProvider.workDoneProgress`: code-action requests may report progress. lsp-cli can surface it.
- `codeActionProvider.codeActionKinds`: code action kinds the server may return. lsp-cli can use them to filter or present actions.
- `codeActionProvider.resolveProvider`: code actions can be lazily resolved. lsp-cli can resolve edits only for selected actions.

## `codeLensProvider`

It means: the server supports code lens.
It can be used by lsp-cli this way: implement a future `code-lens` command.

- `codeLensProvider.workDoneProgress`: code-lens requests may report progress. lsp-cli can surface it.
- `codeLensProvider.resolveProvider`: code lenses can be lazily resolved. lsp-cli can resolve only the selected entries.

## `documentLinkProvider`

It means: the server supports document links.
It can be used by lsp-cli this way: implement a future `document-links` command.

- `documentLinkProvider.workDoneProgress`: document-link requests may report progress. lsp-cli can surface it.
- `documentLinkProvider.resolveProvider`: document links can be lazily resolved. lsp-cli can resolve targets on demand.

## `colorProvider`

It means: the server supports document colors and color presentations.
It can be used by lsp-cli this way: implement future color-inspection commands.

- `colorProvider.workDoneProgress`: color requests may report progress. lsp-cli can surface it.
- `colorProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `colorProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `documentFormattingProvider`

It means: the server supports whole-document formatting.
It can be used by lsp-cli this way: implement a future `format` command.

- `documentFormattingProvider.workDoneProgress`: formatting requests may report progress. lsp-cli can surface it.

## `documentRangeFormattingProvider`

It means: the server supports range formatting.
It can be used by lsp-cli this way: implement a future `format-range` command.

- `documentRangeFormattingProvider.workDoneProgress`: range-formatting requests may report progress. lsp-cli can surface it.

## `documentOnTypeFormattingProvider`

It means: the server supports formatting triggered by typing.
It can be used by lsp-cli this way: mostly editor-oriented, but useful if lsp-cli ever simulates on-type formatting.

## `renameProvider`

It means: the server supports rename.
It can be used by lsp-cli this way: implement a future `rename` command.

- `renameProvider.workDoneProgress`: rename requests may report progress. lsp-cli can surface it.
- `renameProvider.prepareProvider`: server supports rename preflight checks. lsp-cli can call `prepareRename` before applying edits.

## `foldingRangeProvider`

It means: the server supports folding ranges.
It can be used by lsp-cli this way: implement a future `folding-ranges` command.

- `foldingRangeProvider.workDoneProgress`: folding-range requests may report progress. lsp-cli can surface it.
- `foldingRangeProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `foldingRangeProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `executeCommandProvider`

It means: the server can execute named commands.
It can be used by lsp-cli this way: implement a future `execute-command` command for server-defined actions.

- `executeCommandProvider.workDoneProgress`: execute-command requests may report progress. lsp-cli can surface it.
- `executeCommandProvider.commands`: commands supported by the server. lsp-cli can list or validate them.

## `selectionRangeProvider`

It means: the server supports selection ranges.
It can be used by lsp-cli this way: implement a future `selection-range` command.

- `selectionRangeProvider.workDoneProgress`: selection-range requests may report progress. lsp-cli can surface it.
- `selectionRangeProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `selectionRangeProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `linkedEditingRangeProvider`

It means: the server supports linked editing ranges.
It can be used by lsp-cli this way: implement a future `linked-editing` inspection command.

- `linkedEditingRangeProvider.workDoneProgress`: linked-editing requests may report progress. lsp-cli can surface it.
- `linkedEditingRangeProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `linkedEditingRangeProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `callHierarchyProvider`

It means: the server supports call hierarchy.
It can be used by lsp-cli this way: this is already used to gate `callers` and `callees`.

- `callHierarchyProvider.workDoneProgress`: call-hierarchy requests may report progress. lsp-cli can surface it.
- `callHierarchyProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `callHierarchyProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `semanticTokensProvider`

It means: the server supports semantic tokens.
It can be used by lsp-cli this way: implement future semantic-token inspection or highlighting commands.

- `semanticTokensProvider.workDoneProgress`: semantic-token requests may report progress. lsp-cli can surface it.
- `semanticTokensProvider.legend`: the token legend. lsp-cli can use it to decode token streams.
- `semanticTokensProvider.legend.tokenTypes`: token types used by the server. lsp-cli can map token ids to names.
- `semanticTokensProvider.legend.tokenModifiers`: token modifiers used by the server. lsp-cli can map modifier ids to names.
- `semanticTokensProvider.range`: server supports range semantic tokens. lsp-cli can choose range queries for smaller scopes.
- `semanticTokensProvider.full`: server supports full-document semantic tokens. lsp-cli can choose full-document queries.
- `semanticTokensProvider.full.delta`: server supports delta updates. lsp-cli can use it in persistent sessions.
- `semanticTokensProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `semanticTokensProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `monikerProvider`

It means: the server supports monikers.
It can be used by lsp-cli this way: implement a future moniker or cross-repository symbol identity command.

- `monikerProvider.workDoneProgress`: moniker requests may report progress. lsp-cli can surface it.
- `monikerProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.

## `typeHierarchyProvider`

It means: the server supports type hierarchy.
It can be used by lsp-cli this way: implement future `supertypes` and `subtypes` commands.

- `typeHierarchyProvider.workDoneProgress`: type-hierarchy requests may report progress. lsp-cli can surface it.
- `typeHierarchyProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `typeHierarchyProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `inlineValueProvider`

It means: the server supports inline values.
It can be used by lsp-cli this way: mostly debugger-oriented; not used today.

- `inlineValueProvider.workDoneProgress`: inline-value requests may report progress. lsp-cli can surface it.
- `inlineValueProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `inlineValueProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `inlayHintProvider`

It means: the server supports inlay hints.
It can be used by lsp-cli this way: implement a future `inlay-hints` command.

- `inlayHintProvider.workDoneProgress`: inlay-hint requests may report progress. lsp-cli can surface it.
- `inlayHintProvider.resolveProvider`: inlay hints can be lazily resolved. lsp-cli can resolve additional fields only when needed.
- `inlayHintProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `inlayHintProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `diagnosticProvider`

It means: the server supports pull diagnostics.
It can be used by lsp-cli this way: implement future document-diagnostic and workspace-diagnostic commands.

- `diagnosticProvider.workDoneProgress`: diagnostic requests may report progress. lsp-cli can surface it.
- `diagnosticProvider.identifier`: diagnostic source id. lsp-cli can use it to label or merge diagnostic streams.
- `diagnosticProvider.interFileDependencies`: diagnostics may depend on other files. lsp-cli can use it when deciding cache invalidation behavior.
- `diagnosticProvider.workspaceDiagnostics`: server supports workspace-wide diagnostics. lsp-cli can choose between document and workspace pulls.
- `diagnosticProvider.documentSelector`: support is scoped to certain documents. lsp-cli can avoid unsupported files.
- `diagnosticProvider.id`: registration id. lsp-cli can track it for dynamic registration.

## `workspaceSymbolProvider`

It means: the server supports workspace symbols.
It can be used by lsp-cli this way: this is already used to gate workspace symbol queries and the commands built on them.

- `workspaceSymbolProvider.workDoneProgress`: workspace-symbol requests may report progress. lsp-cli can surface it for large workspaces.
- `workspaceSymbolProvider.resolveProvider`: workspace symbols can be lazily resolved. lsp-cli can add `workspaceSymbol/resolve` support when needed.

## `workspace`

It means: workspace-scoped server capability container.
It can be used by lsp-cli this way: inspect sub-capabilities before sending workspace-level notifications and requests.

## `workspace.workspaceFolders`

It means: the server supports workspace folders and may request folder changes.
It can be used by lsp-cli this way: answer `workspace/workspaceFolders` correctly and later support folder-change notifications.

- `workspace.workspaceFolders.supported`: server supports workspace folders. lsp-cli can rely on folder-aware behavior.
- `workspace.workspaceFolders.changeNotifications`: server wants folder-change notifications. lsp-cli can send `didChangeWorkspaceFolders` if it ever supports mutable workspaces.

## `workspace.fileOperations`

It means: the server declares interest in file create, rename, and delete operations.
It can be used by lsp-cli this way: send file operation notifications and requests around local filesystem changes.

- `workspace.fileOperations.didCreate`: server wants create notifications. lsp-cli can send them after creating files.
- `workspace.fileOperations.didCreate.filters`: filters for create notifications. lsp-cli can match events before notifying.
- `workspace.fileOperations.didCreate.filters.scheme`: URI scheme filter. lsp-cli can ignore non-matching schemes.
- `workspace.fileOperations.didCreate.filters.pattern`: file pattern filter. lsp-cli can ignore non-matching paths.
- `workspace.fileOperations.didCreate.filters.pattern.glob`: glob expression. lsp-cli can evaluate it against paths.
- `workspace.fileOperations.didCreate.filters.pattern.matches`: target kind filter. lsp-cli can distinguish files vs folders.
- `workspace.fileOperations.didCreate.filters.pattern.options`: matching options. lsp-cli can honor extra filter behavior.
- `workspace.fileOperations.didCreate.filters.pattern.options.ignoreCase`: case-insensitive matching. lsp-cli can use it when evaluating filters.
- `workspace.fileOperations.willCreate`: server wants create preflight requests. lsp-cli can ask the server for edits before creating files.
- `workspace.fileOperations.willCreate.filters`: filters for will-create requests. lsp-cli can match events before requesting.
- `workspace.fileOperations.willCreate.filters.scheme`: URI scheme filter. lsp-cli can ignore non-matching schemes.
- `workspace.fileOperations.willCreate.filters.pattern`: file pattern filter. lsp-cli can ignore non-matching paths.
- `workspace.fileOperations.willCreate.filters.pattern.glob`: glob expression. lsp-cli can evaluate it against paths.
- `workspace.fileOperations.willCreate.filters.pattern.matches`: target kind filter. lsp-cli can distinguish files vs folders.
- `workspace.fileOperations.willCreate.filters.pattern.options`: matching options. lsp-cli can honor extra filter behavior.
- `workspace.fileOperations.willCreate.filters.pattern.options.ignoreCase`: case-insensitive matching. lsp-cli can use it when evaluating filters.
- `workspace.fileOperations.didRename`: server wants rename notifications. lsp-cli can send them after renames.
- `workspace.fileOperations.didRename.filters`: filters for rename notifications. lsp-cli can match events before notifying.
- `workspace.fileOperations.didRename.filters.scheme`: URI scheme filter. lsp-cli can ignore non-matching schemes.
- `workspace.fileOperations.didRename.filters.pattern`: file pattern filter. lsp-cli can ignore non-matching paths.
- `workspace.fileOperations.didRename.filters.pattern.glob`: glob expression. lsp-cli can evaluate it against paths.
- `workspace.fileOperations.didRename.filters.pattern.matches`: target kind filter. lsp-cli can distinguish files vs folders.
- `workspace.fileOperations.didRename.filters.pattern.options`: matching options. lsp-cli can honor extra filter behavior.
- `workspace.fileOperations.didRename.filters.pattern.options.ignoreCase`: case-insensitive matching. lsp-cli can use it when evaluating filters.
- `workspace.fileOperations.willRename`: server wants rename preflight requests. lsp-cli can ask the server for edits before renames.
- `workspace.fileOperations.willRename.filters`: filters for will-rename requests. lsp-cli can match events before requesting.
- `workspace.fileOperations.willRename.filters.scheme`: URI scheme filter. lsp-cli can ignore non-matching schemes.
- `workspace.fileOperations.willRename.filters.pattern`: file pattern filter. lsp-cli can ignore non-matching paths.
- `workspace.fileOperations.willRename.filters.pattern.glob`: glob expression. lsp-cli can evaluate it against paths.
- `workspace.fileOperations.willRename.filters.pattern.matches`: target kind filter. lsp-cli can distinguish files vs folders.
- `workspace.fileOperations.willRename.filters.pattern.options`: matching options. lsp-cli can honor extra filter behavior.
- `workspace.fileOperations.willRename.filters.pattern.options.ignoreCase`: case-insensitive matching. lsp-cli can use it when evaluating filters.
- `workspace.fileOperations.didDelete`: server wants delete notifications. lsp-cli can send them after deletions.
- `workspace.fileOperations.didDelete.filters`: filters for delete notifications. lsp-cli can match events before notifying.
- `workspace.fileOperations.didDelete.filters.scheme`: URI scheme filter. lsp-cli can ignore non-matching schemes.
- `workspace.fileOperations.didDelete.filters.pattern`: file pattern filter. lsp-cli can ignore non-matching paths.
- `workspace.fileOperations.didDelete.filters.pattern.glob`: glob expression. lsp-cli can evaluate it against paths.
- `workspace.fileOperations.didDelete.filters.pattern.matches`: target kind filter. lsp-cli can distinguish files vs folders.
- `workspace.fileOperations.didDelete.filters.pattern.options`: matching options. lsp-cli can honor extra filter behavior.
- `workspace.fileOperations.didDelete.filters.pattern.options.ignoreCase`: case-insensitive matching. lsp-cli can use it when evaluating filters.
- `workspace.fileOperations.willDelete`: server wants delete preflight requests. lsp-cli can ask the server for edits before deletions.
- `workspace.fileOperations.willDelete.filters`: filters for will-delete requests. lsp-cli can match events before requesting.
- `workspace.fileOperations.willDelete.filters.scheme`: URI scheme filter. lsp-cli can ignore non-matching schemes.
- `workspace.fileOperations.willDelete.filters.pattern`: file pattern filter. lsp-cli can ignore non-matching paths.
- `workspace.fileOperations.willDelete.filters.pattern.glob`: glob expression. lsp-cli can evaluate it against paths.
- `workspace.fileOperations.willDelete.filters.pattern.matches`: target kind filter. lsp-cli can distinguish files vs folders.
- `workspace.fileOperations.willDelete.filters.pattern.options`: matching options. lsp-cli can honor extra filter behavior.
- `workspace.fileOperations.willDelete.filters.pattern.options.ignoreCase`: case-insensitive matching. lsp-cli can use it when evaluating filters.

## `experimental`

It means: server-defined experimental capabilities outside the stable core spec.
It can be used by lsp-cli this way: this is already relevant for server-specific extensions such as `serverStatusNotification`, but it should be handled conservatively.

# Client capabilities

## `workspace`

It means: workspace-scoped client capability container.
It can be used by lsp-cli this way: advertise and control support for workspace-level requests, edits, configuration, and file operations.

## `workspace.applyEdit`

It means: the client can apply batch edits sent through `workspace/applyEdit`.
It can be used by lsp-cli this way: this matters if lsp-cli starts executing rename, code action, or refactoring flows that return workspace edits.

## `workspace.workspaceEdit`

It means: details of what kinds of workspace edits the client supports.
It can be used by lsp-cli this way: advertise only the edit shapes that lsp-cli can really apply.

- `workspace.workspaceEdit.documentChanges`: client supports versioned document changes. lsp-cli can advertise it when it can apply structured multi-file edits safely.
- `workspace.workspaceEdit.resourceOperations`: supported create/rename/delete operations. lsp-cli can advertise exactly which file operations it can perform.
- `workspace.workspaceEdit.failureHandling`: failure strategy for partial edit application. lsp-cli can advertise its real rollback behavior.
- `workspace.workspaceEdit.normalizesLineEndings`: client normalizes line endings. lsp-cli can advertise it if it intentionally rewrites line endings.
- `workspace.workspaceEdit.changeAnnotationSupport`: client supports change annotations. lsp-cli can use them to explain edits.
- `workspace.workspaceEdit.changeAnnotationSupport.groupsOnLabel`: client groups equal labels. lsp-cli can advertise it if its output groups edits that way.

## `workspace.didChangeConfiguration`

It means: support for `workspace/didChangeConfiguration`.
It can be used by lsp-cli this way: send configuration changes to long-lived daemon-backed sessions.

- `workspace.didChangeConfiguration.dynamicRegistration`: client supports dynamic registration for config changes. lsp-cli can advertise it if it handles late registration cleanly.

## `workspace.didChangeWatchedFiles`

It means: support for file-watch notifications.
It can be used by lsp-cli this way: send watched-file changes in long-lived sessions instead of relying only on fresh startup.

- `workspace.didChangeWatchedFiles.dynamicRegistration`: client supports dynamic file-watch registration. lsp-cli can advertise it if it tracks registered watchers.
- `workspace.didChangeWatchedFiles.relativePatternSupport`: client supports relative glob patterns. lsp-cli can advertise it if its watcher matching supports them.

## `workspace.symbol`

It means: details of workspace-symbol support on the client side.
It can be used by lsp-cli this way: this is highly relevant because workspace symbol queries are central to existing commands.

- `workspace.symbol.dynamicRegistration`: client supports dynamic workspace-symbol registration. lsp-cli can advertise it if it fully tracks late registrations.
- `workspace.symbol.symbolKind`: symbol kind support container. lsp-cli can advertise broader symbol support than legacy defaults.
- `workspace.symbol.symbolKind.valueSet`: supported symbol kinds. lsp-cli can advertise the full set it can decode and render.
- `workspace.symbol.tagSupport`: client supports symbol tags. lsp-cli can use them to mark deprecated symbols.
- `workspace.symbol.tagSupport.valueSet`: supported symbol tags. lsp-cli can advertise the exact tag set it understands.
- `workspace.symbol.resolveSupport`: client supports lazy workspace symbol resolution. lsp-cli can use it to resolve missing ranges only when needed.
- `workspace.symbol.resolveSupport.properties`: lazily resolvable properties. lsp-cli can advertise which fields it may request later.

## `workspace.executeCommand`

It means: support for `workspace/executeCommand`.
It can be used by lsp-cli this way: enable a future `execute-command` command.

- `workspace.executeCommand.dynamicRegistration`: client supports dynamic execute-command registration. lsp-cli can advertise it if it tracks late registrations.

## `workspace.workspaceFolders`

It means: the client supports workspace folders.
It can be used by lsp-cli this way: this is now advertised and used so `workspace/workspaceFolders` responses stay consistent with `initialize`.

## `workspace.configuration`

It means: the client supports `workspace/configuration` requests.
It can be used by lsp-cli this way: answer server configuration pulls with real config values instead of empty defaults when that becomes useful.

## `workspace.semanticTokens`

It means: workspace-scoped semantic-token refresh support.
It can be used by lsp-cli this way: handle semantic-token refresh requests in long-lived sessions.

- `workspace.semanticTokens.refreshSupport`: client supports semantic-token refresh requests. lsp-cli can advertise it if it invalidates semantic-token caches correctly.

## `workspace.codeLens`

It means: workspace-scoped code-lens refresh support.
It can be used by lsp-cli this way: handle code-lens refresh requests in long-lived sessions.

- `workspace.codeLens.refreshSupport`: client supports code-lens refresh requests. lsp-cli can advertise it if it refreshes code-lens state correctly.

## `workspace.fileOperations`

It means: support for file operation notifications and requests.
It can be used by lsp-cli this way: notify servers about local filesystem changes and ask for preflight edits.

- `workspace.fileOperations.dynamicRegistration`: client supports dynamic file operation registration. lsp-cli can advertise it if it tracks registrations and filters.
- `workspace.fileOperations.didCreate`: client can send create notifications. lsp-cli can advertise it if file creation flows send `didCreateFiles`.
- `workspace.fileOperations.willCreate`: client can send create preflight requests. lsp-cli can advertise it if it asks the server before file creation.
- `workspace.fileOperations.didRename`: client can send rename notifications. lsp-cli can advertise it if rename flows notify the server.
- `workspace.fileOperations.willRename`: client can send rename preflight requests. lsp-cli can advertise it if it asks the server before renames.
- `workspace.fileOperations.didDelete`: client can send delete notifications. lsp-cli can advertise it if delete flows notify the server.
- `workspace.fileOperations.willDelete`: client can send delete preflight requests. lsp-cli can advertise it if it asks the server before deletions.

## `workspace.inlineValue`

It means: workspace-scoped inline-value refresh support.
It can be used by lsp-cli this way: not used today, but relevant if notebook/debugger-oriented LSP features are added.

- `workspace.inlineValue.refreshSupport`: client supports inline-value refresh requests. lsp-cli can advertise it if it refreshes inline values correctly.

## `workspace.inlayHint`

It means: workspace-scoped inlay-hint refresh support.
It can be used by lsp-cli this way: refresh inlay-hint views in long-lived sessions.

- `workspace.inlayHint.refreshSupport`: client supports inlay-hint refresh requests. lsp-cli can advertise it if it refreshes inlay hints correctly.

## `workspace.diagnostics`

It means: workspace-scoped diagnostics refresh support.
It can be used by lsp-cli this way: support server-triggered diagnostic refreshes in long-lived sessions.

- `workspace.diagnostics.refreshSupport`: client supports diagnostics refresh requests. lsp-cli can advertise it if it refetches diagnostics correctly.

## `textDocument`

It means: text-document-scoped client capability container.
It can be used by lsp-cli this way: advertise feature-specific support for text-document requests.

## `textDocument.synchronization`

It means: text document synchronization support on the client.
It can be used by lsp-cli this way: advertise how much document lifecycle and save traffic lsp-cli can send.

- `textDocument.synchronization.dynamicRegistration`: client supports dynamic sync registration. lsp-cli can advertise it if it handles late sync registration correctly.
- `textDocument.synchronization.willSave`: client sends will-save notifications. lsp-cli can advertise it if save flows emit them.
- `textDocument.synchronization.willSaveWaitUntil`: client sends will-save-wait-until requests. lsp-cli can advertise it if save flows can apply returned edits.
- `textDocument.synchronization.didSave`: client sends did-save notifications. lsp-cli can advertise it if save flows emit them.

## `textDocument.completion`

It means: completion support and output formats accepted by the client.
It can be used by lsp-cli this way: enable a future completion feature without over-claiming UI-specific support.

- `textDocument.completion.dynamicRegistration`: client supports dynamic completion registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.completion.completionItem`: completion item support container. lsp-cli can inspect or advertise nested completion item support.
- `textDocument.completion.completionItem.snippetSupport`: client supports snippet insert text. lsp-cli should advertise it only if it can preserve or render snippets meaningfully.
- `textDocument.completion.completionItem.commitCharactersSupport`: client supports commit characters. lsp-cli can use them when applying selected completion items.
- `textDocument.completion.completionItem.documentationFormat`: accepted markup formats. lsp-cli can advertise markdown/plaintext formats it renders correctly.
- `textDocument.completion.completionItem.deprecatedSupport`: client supports deprecated completion items. lsp-cli can mark them in output.
- `textDocument.completion.completionItem.preselectSupport`: client supports preselected completion items. lsp-cli can respect that when ranking.
- `textDocument.completion.completionItem.tagSupport`: client supports completion item tags. lsp-cli can use them for deprecated entries.
- `textDocument.completion.completionItem.tagSupport.valueSet`: supported completion item tags. lsp-cli can advertise the exact tag set it understands.
- `textDocument.completion.completionItem.insertReplaceSupport`: client supports insert-replace edits. lsp-cli can advertise it if it can apply both insertion and replacement ranges.
- `textDocument.completion.completionItem.resolveSupport`: client supports lazy completion item resolution. lsp-cli can resolve docs or details on demand.
- `textDocument.completion.completionItem.resolveSupport.properties`: lazily resolvable completion fields. lsp-cli can advertise only the properties it may request later.
- `textDocument.completion.completionItem.insertTextModeSupport`: client supports `insertTextMode`. lsp-cli can advertise it if it applies those modes correctly.
- `textDocument.completion.completionItem.insertTextModeSupport.valueSet`: supported insert text modes. lsp-cli can advertise the exact modes it supports.
- `textDocument.completion.completionItem.labelDetailsSupport`: client supports label details. lsp-cli can render richer completion labels.
- `textDocument.completion.completionItemKind`: completion item kind support container. lsp-cli can advertise broad kind support.
- `textDocument.completion.completionItemKind.valueSet`: supported completion item kinds. lsp-cli can advertise the full set it renders.
- `textDocument.completion.contextSupport`: client can send completion context. lsp-cli can use it for trigger-aware completion requests.
- `textDocument.completion.insertTextMode`: default insert text mode. lsp-cli can advertise its default insertion behavior.
- `textDocument.completion.completionList`: completion list support container. lsp-cli can advertise list-level defaults if it supports them.
- `textDocument.completion.completionList.itemDefaults`: supported `CompletionList.itemDefaults` properties. lsp-cli can advertise only what it can apply.

## `textDocument.hover`

It means: hover support and accepted content formats.
It can be used by lsp-cli this way: enable a future `hover` command.

- `textDocument.hover.dynamicRegistration`: client supports dynamic hover registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.hover.contentFormat`: accepted hover markup formats. lsp-cli can advertise markdown/plaintext formats it renders correctly.

## `textDocument.signatureHelp`

It means: signature-help support and accepted output formats.
It can be used by lsp-cli this way: enable a future `signature-help` command.

- `textDocument.signatureHelp.dynamicRegistration`: client supports dynamic signature-help registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.signatureHelp.signatureInformation`: signature information support container. lsp-cli can advertise nested support accurately.
- `textDocument.signatureHelp.signatureInformation.documentationFormat`: accepted markup formats. lsp-cli can advertise markdown/plaintext formats it renders correctly.
- `textDocument.signatureHelp.signatureInformation.parameterInformation`: parameter info support container. lsp-cli can advertise nested support accurately.
- `textDocument.signatureHelp.signatureInformation.parameterInformation.labelOffsetSupport`: client supports label offsets. lsp-cli can advertise it if it can map offset ranges back into labels.
- `textDocument.signatureHelp.signatureInformation.activeParameterSupport`: client supports `activeParameter`. lsp-cli can highlight active parameters in output.
- `textDocument.signatureHelp.contextSupport`: client sends signature-help context. lsp-cli can use it in trigger-aware interactive flows.

## `textDocument.declaration`

It means: declaration request support.
It can be used by lsp-cli this way: this is valuable because lsp-cli already parses `LocationLink` and can benefit from precise declaration results.

- `textDocument.declaration.dynamicRegistration`: client supports dynamic declaration registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.declaration.linkSupport`: client accepts `LocationLink[]` for declarations. lsp-cli should likely advertise it because it already parses links.

## `textDocument.definition`

It means: definition request support.
It can be used by lsp-cli this way: this is valuable because lsp-cli already parses `LocationLink` and can benefit from precise definition results.

- `textDocument.definition.dynamicRegistration`: client supports dynamic definition registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.definition.linkSupport`: client accepts `LocationLink[]` for definitions. lsp-cli should likely advertise it because it already parses links.

## `textDocument.typeDefinition`

It means: type-definition request support.
It can be used by lsp-cli this way: enable a future `type-definition` command with precise link results.

- `textDocument.typeDefinition.dynamicRegistration`: client supports dynamic type-definition registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.typeDefinition.linkSupport`: client accepts `LocationLink[]` for type definitions. lsp-cli can advertise it when it implements the feature.

## `textDocument.implementation`

It means: implementation request support.
It can be used by lsp-cli this way: enable a future `implementation` command with precise link results.

- `textDocument.implementation.dynamicRegistration`: client supports dynamic implementation registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.implementation.linkSupport`: client accepts `LocationLink[]` for implementations. lsp-cli can advertise it when it implements the feature.

## `textDocument.references`

It means: references request support.
It can be used by lsp-cli this way: complements the existing `references` command.

- `textDocument.references.dynamicRegistration`: client supports dynamic references registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.documentHighlight`

It means: document-highlight request support.
It can be used by lsp-cli this way: enable a future `document-highlight` command.

- `textDocument.documentHighlight.dynamicRegistration`: client supports dynamic document-highlight registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.documentSymbol`

It means: document-symbol request support and accepted symbol shapes.
It can be used by lsp-cli this way: this is highly relevant because document symbols are already central to multiple commands.

- `textDocument.documentSymbol.dynamicRegistration`: client supports dynamic document-symbol registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.documentSymbol.symbolKind`: document symbol kind support container. lsp-cli can advertise broad symbol kind support.
- `textDocument.documentSymbol.symbolKind.valueSet`: supported symbol kinds. lsp-cli can advertise the full set it decodes.
- `textDocument.documentSymbol.hierarchicalDocumentSymbolSupport`: client supports nested `DocumentSymbol`. lsp-cli should likely advertise it because it already parses nested symbols.
- `textDocument.documentSymbol.tagSupport`: client supports symbol tags. lsp-cli can use them to mark deprecated symbols.
- `textDocument.documentSymbol.tagSupport.valueSet`: supported symbol tags. lsp-cli can advertise the exact tag set it understands.
- `textDocument.documentSymbol.labelSupport`: client supports provider labels. lsp-cli can advertise it if it renders provider identity.

## `textDocument.codeAction`

It means: code-action request support and accepted output shapes.
It can be used by lsp-cli this way: enable future code action discovery and application flows.

- `textDocument.codeAction.dynamicRegistration`: client supports dynamic code-action registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.codeAction.codeActionLiteralSupport`: client accepts code action literals. lsp-cli can advertise it if it can decode and present them.
- `textDocument.codeAction.codeActionLiteralSupport.codeActionKind`: code action kind support container. lsp-cli can advertise the kinds it understands.
- `textDocument.codeAction.codeActionLiteralSupport.codeActionKind.valueSet`: supported code action kinds. lsp-cli can advertise the exact kind set it understands.
- `textDocument.codeAction.isPreferredSupport`: client supports `isPreferred`. lsp-cli can use it for ranking.
- `textDocument.codeAction.disabledSupport`: client supports `disabled`. lsp-cli can render disabled actions and reasons.
- `textDocument.codeAction.dataSupport`: client preserves `data` across resolve calls. lsp-cli can advertise it if it forwards unresolved items correctly.
- `textDocument.codeAction.resolveSupport`: client supports lazy code action resolution. lsp-cli can resolve edits only for chosen actions.
- `textDocument.codeAction.resolveSupport.properties`: lazily resolvable fields. lsp-cli can advertise only what it may request later.
- `textDocument.codeAction.honorsChangeAnnotations`: client honors change annotations. lsp-cli can advertise it if it preserves them when applying edits.

## `textDocument.codeLens`

It means: code-lens request support.
It can be used by lsp-cli this way: enable a future `code-lens` command.

- `textDocument.codeLens.dynamicRegistration`: client supports dynamic code-lens registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.documentLink`

It means: document-link request support.
It can be used by lsp-cli this way: enable a future `document-links` command.

- `textDocument.documentLink.dynamicRegistration`: client supports dynamic document-link registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.documentLink.tooltipSupport`: client supports link tooltips. lsp-cli can advertise it if it renders tooltip text.

## `textDocument.colorProvider`

It means: document color support on the client side.
It can be used by lsp-cli this way: enable future color-inspection commands.

- `textDocument.colorProvider.dynamicRegistration`: client supports dynamic color-provider registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.formatting`

It means: whole-document formatting support.
It can be used by lsp-cli this way: enable a future `format` command.

- `textDocument.formatting.dynamicRegistration`: client supports dynamic formatting registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.rangeFormatting`

It means: range formatting support.
It can be used by lsp-cli this way: enable a future `format-range` command.

- `textDocument.rangeFormatting.dynamicRegistration`: client supports dynamic range-formatting registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.onTypeFormatting`

It means: on-type formatting support.
It can be used by lsp-cli this way: mainly useful for editor-like integrations.

- `textDocument.onTypeFormatting.dynamicRegistration`: client supports dynamic on-type-formatting registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.rename`

It means: rename support and rename output handling details.
It can be used by lsp-cli this way: enable a future `rename` command that validates and applies workspace edits.

- `textDocument.rename.dynamicRegistration`: client supports dynamic rename registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.rename.prepareSupport`: client supports `prepareRename`. lsp-cli can use it to preflight rename requests.
- `textDocument.rename.prepareSupportDefaultBehavior`: client supports default-behavior rename prepare results. lsp-cli can use it if it fully implements that branch.
- `textDocument.rename.honorsChangeAnnotations`: client honors change annotations in rename edits. lsp-cli can advertise it if it preserves annotations while applying edits.

## `textDocument.publishDiagnostics`

It means: support for pushed diagnostics.
It can be used by lsp-cli this way: enable future diagnostics commands or daemon-side diagnostic caching.

- `textDocument.publishDiagnostics.relatedInformation`: client accepts related diagnostic info. lsp-cli can render it in output.
- `textDocument.publishDiagnostics.tagSupport`: client supports diagnostic tags. lsp-cli can render deprecated or unnecessary markers.
- `textDocument.publishDiagnostics.tagSupport.valueSet`: supported diagnostic tags. lsp-cli can advertise the exact tag set it understands.
- `textDocument.publishDiagnostics.versionSupport`: client honors diagnostic versions. lsp-cli can use versions to avoid stale results.
- `textDocument.publishDiagnostics.codeDescriptionSupport`: client supports diagnostic `codeDescription`. lsp-cli can render doc links for diagnostics.
- `textDocument.publishDiagnostics.dataSupport`: client preserves diagnostic `data`. lsp-cli can advertise it if it forwards diagnostics into code-action flows.

## `textDocument.foldingRange`

It means: folding-range support and accepted range variants.
It can be used by lsp-cli this way: enable a future `folding-ranges` command.

- `textDocument.foldingRange.dynamicRegistration`: client supports dynamic folding-range registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.foldingRange.rangeLimit`: preferred maximum number of ranges. lsp-cli can advertise a practical bound if needed.
- `textDocument.foldingRange.lineFoldingOnly`: client supports only whole-line folding. lsp-cli can advertise it if it cannot represent character-level folding.
- `textDocument.foldingRange.foldingRangeKind`: supported folding-range kinds container. lsp-cli can advertise the kinds it understands.
- `textDocument.foldingRange.foldingRangeKind.valueSet`: supported folding kinds. lsp-cli can advertise the exact set it understands.
- `textDocument.foldingRange.foldingRange`: folding-range feature container. lsp-cli can advertise nested support accurately.
- `textDocument.foldingRange.foldingRange.collapsedText`: client supports custom collapsed text. lsp-cli can advertise it if it renders that text.

## `textDocument.selectionRange`

It means: selection-range support.
It can be used by lsp-cli this way: enable a future `selection-range` command.

- `textDocument.selectionRange.dynamicRegistration`: client supports dynamic selection-range registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.linkedEditingRange`

It means: linked-editing-range support.
It can be used by lsp-cli this way: enable a future linked-editing inspection command.

- `textDocument.linkedEditingRange.dynamicRegistration`: client supports dynamic linked-editing registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.callHierarchy`

It means: call-hierarchy support.
It can be used by lsp-cli this way: this directly complements existing `callers` and `callees` support.

- `textDocument.callHierarchy.dynamicRegistration`: client supports dynamic call-hierarchy registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.semanticTokens`

It means: semantic-token support and accepted request/result variants.
It can be used by lsp-cli this way: enable future semantic-token inspection commands.

- `textDocument.semanticTokens.dynamicRegistration`: client supports dynamic semantic-token registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.semanticTokens.requests`: semantic-token request support container. lsp-cli can advertise which request forms it supports.
- `textDocument.semanticTokens.requests.range`: client supports range token requests. lsp-cli can use smaller-scope queries.
- `textDocument.semanticTokens.requests.full`: client supports full-document token requests. lsp-cli can use full-document queries.
- `textDocument.semanticTokens.requests.full.delta`: client supports delta token requests. lsp-cli can use them in persistent sessions.
- `textDocument.semanticTokens.tokenTypes`: supported token types. lsp-cli can advertise the full set it can decode.
- `textDocument.semanticTokens.tokenModifiers`: supported token modifiers. lsp-cli can advertise the full set it can decode.
- `textDocument.semanticTokens.formats`: supported wire formats. lsp-cli can advertise only formats it parses.
- `textDocument.semanticTokens.overlappingTokenSupport`: client supports overlapping tokens. lsp-cli can advertise it if its renderer tolerates overlaps.
- `textDocument.semanticTokens.multilineTokenSupport`: client supports multi-line tokens. lsp-cli can advertise it if its decoder tolerates them.
- `textDocument.semanticTokens.serverCancelSupport`: client supports server cancellation of semantic-token requests. lsp-cli can advertise it if it handles those errors correctly.
- `textDocument.semanticTokens.augmentsSyntaxTokens`: semantic tokens augment syntax tokens. lsp-cli can advertise it if it combines both models.

## `textDocument.moniker`

It means: moniker support.
It can be used by lsp-cli this way: enable a future moniker command.

- `textDocument.moniker.dynamicRegistration`: client supports dynamic moniker registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.typeHierarchy`

It means: type-hierarchy support.
It can be used by lsp-cli this way: enable future `supertypes` and `subtypes` commands.

- `textDocument.typeHierarchy.dynamicRegistration`: client supports dynamic type-hierarchy registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.inlineValue`

It means: inline-value support.
It can be used by lsp-cli this way: mostly debugger-oriented and not used today.

- `textDocument.inlineValue.dynamicRegistration`: client supports dynamic inline-value registration. lsp-cli can advertise it if it tracks late registrations.

## `textDocument.inlayHint`

It means: inlay-hint support.
It can be used by lsp-cli this way: enable a future `inlay-hints` command.

- `textDocument.inlayHint.dynamicRegistration`: client supports dynamic inlay-hint registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.inlayHint.resolveSupport`: client supports lazy inlay-hint resolution. lsp-cli can resolve extra hint fields only when needed.
- `textDocument.inlayHint.resolveSupport.properties`: lazily resolvable fields. lsp-cli can advertise only the properties it may request later.

## `textDocument.diagnostic`

It means: pull-diagnostic support on the client side.
It can be used by lsp-cli this way: enable future `document-diagnostic` and `workspace-diagnostic` commands.

- `textDocument.diagnostic.dynamicRegistration`: client supports dynamic diagnostic registration. lsp-cli can advertise it if it tracks late registrations.
- `textDocument.diagnostic.relatedDocumentSupport`: client supports related-document diagnostics. lsp-cli can render diagnostics that span multiple documents.
- `textDocument.diagnostic.relatedInformation`: client accepts related diagnostic information. lsp-cli can render it in output.
- `textDocument.diagnostic.tagSupport`: client supports diagnostic tags. lsp-cli can render deprecated or unnecessary markers.
- `textDocument.diagnostic.tagSupport.valueSet`: supported diagnostic tags. lsp-cli can advertise the exact tag set it understands.
- `textDocument.diagnostic.codeDescriptionSupport`: client supports diagnostic `codeDescription`. lsp-cli can render documentation links.
- `textDocument.diagnostic.dataSupport`: client preserves diagnostic `data`. lsp-cli can advertise it if it forwards diagnostics into later flows.

## `notebookDocument`

It means: notebook-document-scoped client capability container.
It can be used by lsp-cli this way: only if notebook support is added.

## `notebookDocument.synchronization`

It means: notebook synchronization support.
It can be used by lsp-cli this way: only if notebook support is added.

- `notebookDocument.synchronization.dynamicRegistration`: client supports dynamic notebook sync registration. lsp-cli can advertise it if it tracks late registrations.
- `notebookDocument.synchronization.executionSummarySupport`: client can send cell execution summaries. lsp-cli can advertise it only if it really has notebook execution data.

## `window`

It means: window-scoped client capability container.
It can be used by lsp-cli this way: advertise progress and UI request support carefully, because lsp-cli is not a full editor UI.

## `window.workDoneProgress`

It means: the client supports server-initiated work-done progress.
It can be used by lsp-cli this way: this is already used for `wait-for-index` and `build-index` flows.

## `window.showMessage`

It means: support for richer `window/showMessageRequest` behavior.
It can be used by lsp-cli this way: return richer action items if lsp-cli ever wants to preserve custom fields.

- `window.showMessage.messageActionItem`: message action item support container. lsp-cli can advertise nested support accurately.
- `window.showMessage.messageActionItem.additionalPropertiesSupport`: client preserves extra action item properties. lsp-cli can advertise it if it round-trips them instead of dropping them.

## `window.showDocument`

It means: support for `window/showDocument`.
It can be used by lsp-cli this way: support server requests to open or reveal documents in an external tool or a textual report.

- `window.showDocument.support`: client supports `window/showDocument`. lsp-cli can advertise it only if it implements a meaningful action.

## `general`

It means: general client capability container.
It can be used by lsp-cli this way: advertise cross-cutting protocol behavior such as encoding, regex engine, markdown parser, and stale request handling.

## `general.staleRequestSupport`

It means: how the client treats stale requests and stale results.
It can be used by lsp-cli this way: useful for long-lived interactive sessions, but less important for short one-shot commands.

- `general.staleRequestSupport.cancel`: client actively cancels stale requests. lsp-cli can advertise it if it implements request cancellation.
- `general.staleRequestSupport.retryOnContentModified`: requests retried after `ContentModified`. lsp-cli can advertise it if it has retry logic.

## `general.regularExpressions`

It means: the regex engine used by the client.
It can be used by lsp-cli this way: relevant for capabilities that depend on the client regex engine.

- `general.regularExpressions.engine`: regex engine name. lsp-cli can advertise the actual engine it expects servers to target.
- `general.regularExpressions.version`: regex engine version. lsp-cli can advertise the actual supported version.

## `general.markdown`

It means: the markdown parser and HTML policy used by the client.
It can be used by lsp-cli this way: advertise only the markdown features that lsp-cli output really preserves.

- `general.markdown.parser`: markdown parser name. lsp-cli can advertise the parser semantics it relies on.
- `general.markdown.version`: parser version. lsp-cli can advertise the actual version if relevant.
- `general.markdown.allowedTags`: allowed HTML tags in markdown. lsp-cli can advertise the exact sanitization policy it uses.

## `general.positionEncodings`

It means: the position encodings the client supports.
It can be used by lsp-cli this way: this is already used, and lsp-cli currently advertises only UTF-16.

## `experimental`

It means: client-defined experimental capabilities outside the stable core spec.
It can be used by lsp-cli this way: this is already used for `serverStatusNotification` to interoperate with servers such as `rust-analyzer`.
