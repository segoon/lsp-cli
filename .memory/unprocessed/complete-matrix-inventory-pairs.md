# Complete E2E matrix must not duplicate compatibility data

The initial implementation copied every compatible `language/server` relation from
`data/lsp/*.yaml` into bare `tests/e2e/cases/*.yaml` entries. The user pointed out that this creates
two sources of truth. Complete inventory should instead be resolved directly from the pinned data;
case YAML should contain only E2E-specific behavior and reviewed exclusions.

Bare pair entries are now invalid. `E2E_CASE` selects only configured cases, so selecting a
compatible pair that has no E2E behavior cannot silently run zero tests.
