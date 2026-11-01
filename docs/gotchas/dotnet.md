# .NET language servers

## roslyn-language-server

- The Mason package exposes Roslyn through the `roslyn-language-server` .NET tool launcher. A data
  config containing a literal installation placeholder cannot work with generic `--download`;
  launch the Mason-exposed command and let the NuGet backend manage its concrete installation path.
- Roslyn 5.12 returns namespace-qualified type names and parenthesized method names from
  `textDocument/documentSymbol`. E2E schema v13 permits pair-local exact callable and expected-name
  overrides so tests retain Roslyn's output without changing production results or weakening
  OmniSharp assertions. Roslyn still returns no workspace symbols, references, or call-hierarchy
  edges for the shared fixture and exposes no terminal background-index signal; those results are
  asserted as narrow query exceptions.

## OmniSharp

- OmniSharp 1.39.15 advertises formatting, document symbols, references, definition,
  implementation, and type-definition providers as options objects (`{}`). These count as
  supported capabilities. Both C# and Visual Basic contracts must include them; querying
  type-definition for the `BuildSampleOrder` method still returns no matches, so that non-applicable
  relationship has a narrow empty-result exception, like the Roslyn method query.
- OmniSharp 1.39.15 previously closed the transport while lsp-cli waited for a `shutdown` response.
  Against pinned Mason snapshot `2026-09-30-aboard-mob`, however, the complete C# smoke case passed
  twice in fresh isolated homes, including repeated direct shutdown exchanges. Its stale broad
  expected-failure marker was removed; the dedicated lifecycle scenario remains the narrower place
  to detect a recurrence.
- Full-suite validation with snapshot `2026-10-01-remote-route` reproduced this shutdown closure
  for `pkg:github/OmniSharp/omnisharp-roslyn@v1.39.15` during the initial C# capability query.
  Descendant cleanup and removal of both isolated roots succeeded; the protocol failure remains
  separate from orphan-process cleanup and has not been added back to expected failures.
- Investigation with snapshot `2026-10-25-chummy-chest` reproduced the apparent shutdown
  closure twice in three capability commands against OmniSharp 1.39.15. A direct protocol probe
  identified an occasional framed JSON `null` between project-status notifications. Deserializing
  its body into `Option<Value>` incorrectly treats that payload as EOF even while OmniSharp is
  alive. The transport must parse a `Value` first and wrap it in `Some`, then keep waiting for the
  actual shutdown response. A framed `null` is not a valid JSON-RPC response and must not satisfy
  the pending request; no shutdown delay or relaxed EOF policy is needed. Twenty repeated
  capability commands passed after the fix, including two runs that received null frames.

