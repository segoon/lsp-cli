# Position-based navigation

## Final decisions

Support all seven navigation commands: definition, declaration, type-definition,
implementation, references, callers, and callees.

Three input forms:

```sh
lsp-cli definition NAME [DIRECTORY]
lsp-cli definition --at FILE:LINE:COLUMN
lsp-cli definition --at FILE:LINE:COLUMN NAME
```

Name queries default to the current directory and preserve explicit-directory
usage. Position queries infer language/workspace context from the file, preserving
language/server overrides. Directory arguments are restricted to name mode.
Locations are parsed from the right so paths may contain colons.

The user selected “UTF-8 everywhere,” continuing the discussed interpretation of
one-based Unicode scalar columns, for both input and all text/JSON output. Tabs
count as one scalar; combining marks count separately. LSP positions remain
zero-based UTF-16 units internally. Printed non-ASCII columns therefore change
from the previous output convention, requiring consumers to adapt.

With both inputs, assert exact, case-sensitive text starting at the cursor's first
character. This is a literal prefix check, not language-specific identifier or
semantic validation: `foo` matches the start of `foobar`. Fail on mismatch before
server startup, and never fall back to discovery. The assertion does not detect
moving to another occurrence with the same text.

Reject zero/out-of-range coordinates. Allow an insertion position immediately
after the last character, including empty lines and the final empty line following
a newline. Resolve relative file paths from the current directory.

## Shared implementation

Share CLI option groups, target parsing/validation, anchor resolution, session
orchestration, deduplication, and rendering. Retain the two distinct protocol
flows: direct location requests and prepared call hierarchy.

Represent name targets and position targets explicitly. Position targets bypass
symbol discovery and its capability/callable-filter requirements. Separate query
anchors from symbol results so an explicit source position requires no invented
symbol metadata. Known metadata remains intact; unavailable name/kind fields are
JSON null. Asserted text supplies a fallback name for location results. Position
JSON retains the location string as `query` and adds `at` and `expected_name`.

Use existing source-line conversion abstractions and source caching. All coordinate
output uses scalar columns, including diagnostic start/end positions. Missing
source or invalid server positions are errors; path-only output needs no conversion.

This avoids seven feature implementations and leaves a target representation for
future position operations. Keep listing/diagnostic purposes unchanged, apart from
consistent printed coordinates. No dependencies or language-specific parsing are
needed. A broader universal command framework would add unnecessary coupling.

## Benefits and limitations

Positions disambiguate same-named symbols, select usage context, reach occurrences
omitted by symbol discovery, and avoid discovery scans. Server initialization and
analysis still take time. Coordinates become stale when files change. Exact positions
can still yield multiple semantic targets. For call hierarchy, identify the callable;
an arbitrary position in its body need not select the enclosing function.

## Questions and user answers

- Input forms: the user specified name, position, and position plus name assertion.
- Cursor placement: asked first character versus anywhere; user chose first character.
- Coordinates and command scope: user chose “UTF-8 everywhere” and “all seven.”
- Workspace handling: user chose an optional directory defaulting to `.` for names,
  with position queries deriving context from the file.
- Missing metadata: user chose null without extra enrichment requests.
- Failed coordinate conversion: user chose a user-facing error.

## Validation

Cover all three forms and all seven commands in shared CLI/protocol tests and
mock E2E scenarios. Exercise usage-site and callable positions against the real
Rust playground. Test Unicode, CRLF, insertion positions, assertions, unknown
metadata, discovery bypass, diagnostic endpoints, and source-conversion failures.
Run `make test`, format/Clippy/generated-documentation checks, and the complete
real-server suite after E2E runner changes.
