# pylyzer

- pylyzer 0.0.82 can return generated `__pycache__/*.d.er` files from `workspace/symbol`
  when a named Python query falls back from document symbols. Strict document-language
  resolution previously rejected these Erg locations before sending `textDocument/typeDefinition`,
  since they do not match the selected Python filetype. CI run `41336328999` and a local replay
  reproduced this with different generated files. Query locations now carry their origin:
  unknown-language server locations are queried without `didOpen`, while explicit user
  locations remain strict. The known empty type-definition result keeps its narrow
  expected-failure diagnostic; no server-specific language guess or cache-path exclusion is needed.
- A direct protocol probe returned broad candidates for workspace query `order`, including
  17 generated-file locations without an exact `order` symbol. Sending type-definition
  requests to all 17 locations without `didOpen` succeeded with empty results. This supports
  querying server-returned unknown-language files without taking ownership of their content;
  the separate policy of accepting broad candidates remains relevant when no exact name matches.
- In an isolated current-Mason run against the Python playground, pylyzer returned no immediate
  `workspace/symbol` matches and no incoming call-hierarchy edges for `build_sample_order`, while
  its other applicable semantic queries completed. Exhaustive coverage records those two empty
  results explicitly instead of treating advertised capabilities as a guarantee of fixture matches.
- Mason package `pkg:github/mtshiba/pylyzer@v0.0.82` also returns no
  `textDocument/typeDefinition` match for an `order` parameter explicitly typed as `Order`, after
  the same fixture passes that query with BasedPyright and pyright. This is a pylyzer compatibility
  limitation rather than a missing fixture relationship. Separately, the server repeatedly reports
  a missing `ERG_PATH` and diagnostics-worker index-out-of-bounds panics; semantic replies can still
  succeed, so those stderr messages are not currently treated as the query's direct failure.

