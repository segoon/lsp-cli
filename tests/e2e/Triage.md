# Triage an E2E CI failure

Pairs use `<language>/<server-config-id>`, e.g. `python/pyright`. The server component is the
filename stem under `data/lsp/`, not necessarily the executable or display name.

1. Open the workflow summary and find the pair's executable or excluded classification.
2. Open the failed `<language>/<installation-family>` matrix job.
3. Read the `E2E failed case IDs` footer, which lists the specific failed pairs collected from the
   diagnostics above it. If the footer says the IDs are unavailable, failure occurred before a case
   emitted its labelled diagnostic — start with the planner, build, or test-runner error
   immediately above the footer.

Each matrix job also uploads `e2e-results-<language>-<installation-family>`. Its JSON document has
`schema_version: 1`, one result per executed case, and aggregate pass/failure counts. A failed case
records its human-readable diagnostic and one of these stable execution stages: `setup`,
`provisioning`, `capabilities`, `query`, `lifecycle`, or `cleanup`. The stage identifies where the
E2E harness observed the failure; it is not a substitute for diagnosing the underlying cause.

The job summary contains the same aggregate stage counts. If no result file was produced, the test
binary failed before it could finish a labelled case; the summary says to inspect the job log rather
than inventing a case classification.

### Identify the upstream version

A failed case retains this block before deleting its isolated home:

```text
server package source IDs:
pkg:npm/pyright@1.1.409
server command line:
...
server capabilities:
...
server stderr summary:
...
cleanup state:
...
```

Treat each complete `pkg:<installation-family>/<package>@<version>` source ID as the authoritative
package identity — preserve the whole value, since versions and package names can contain
prefixes, scopes, or backend-specific suffixes. The executable name and `initialize` response
version may describe a product differently and are supporting evidence, not replacements for the
source ID.

`<unavailable: no completed server installation>` normally means provisioning failed before a
receipt was written; inspect the preceding download/install error rather than inferring a version
from an older run. Outside the isolated suite, successful downloads store JSON receipts under
`~/.local/share/lsp-cli/receipts/` (same `source_id` field meaning). E2E case homes and their
receipts are removed after confirmed descendant cleanup, including failed cases. Incomplete
cleanup retains the roots; consult the final cleanup diagnostic before using a logged path.

Successful E2E cases print no retained failure context. The suite follows Mason latest, so a later
rerun may resolve a different source ID — compare IDs from available failing runs and record the ID
in an issue when exact upstream identity matters; the suite does not promise the registry will
retain an older version for reproduction.

### Classify the failure

Check evidence in this order:

1. **Planner or manifest:** an unknown selector, missing registry package, or validation failure
   happened before a server case ran.
2. **Provisioning or network:** no completed receipt, package-manager output, HTTP failure, or an
   absent host program points to installation rather than LSP behavior — use
   `make test-e2e PHASE=provision`.
3. **Startup or shutdown:** use the retained command line and server stderr. SDK incompatibility,
   launcher failure, crash, and failure to exit are distinct from query-result drift.
4. **Protocol or capability:** compare the retained capabilities with the command exercised by the
   case. An unadvertised optional capability passes only when lsp-cli returns its expected
   user-facing unsupported error.
5. **Semantic result:** compare stable names and the pair's reviewed exceptions (see
   [Real-server exceptions](Readme.md#real-server-exceptions)). Do not weaken an expectation until
   the same source ID reproduces the behavior or an upstream change is confirmed.
6. **Cleanup:** inspect sandbox and runtime roots independently of the primary failure — a
   successful query with a retained root is still a cleanup regression.
