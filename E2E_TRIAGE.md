# E2E compatibility triage

This guide is for maintainers investigating the scheduled **End-to-end compatibility** workflow
or the real-server pull-request job. These tests install the current package selected by the Mason
registry, so an unchanged lsp-cli commit can fail after an upstream release.

## Find the failing pair

Pairs use `<language>/<server-config-id>`, for example `python/pyright`. The server component is the
filename stem under `data/lsp/`, not necessarily the executable or display name.

1. Open the workflow summary and find the pair's executable or excluded classification.
2. Open the failed `<language>/<installation-family>` matrix job.
3. Read the `E2E failed case IDs` footer. It lists the specific failed pairs collected from the
   longer diagnostics above it.

If the footer says the IDs are unavailable, failure occurred before a case emitted its labelled
diagnostic. Start with the planner, build, or test-runner error immediately above the footer.

## Identify the upstream version

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
package identity. Preserve the whole value: versions and package names can contain prefixes,
scopes, or backend-specific suffixes. The executable name and `initialize` response version may
describe a product differently and are supporting evidence, not replacements for the source ID.

`<unavailable: no completed server installation>` normally means provisioning failed before a
receipt was written. A missing or malformed receipt is reported separately. Inspect the preceding
download/install error and do not infer a version from an older run.

Outside the isolated suite, successful downloads store JSON receipts under
`~/.local/share/lsp-cli/receipts/`; the `source_id` field has the same meaning. E2E case homes and
their receipts are intentionally removed after every case, including failures, so use the retained
diagnostic rather than a path printed earlier in the log.

Successful E2E cases do not print retained failure context. The suite follows Mason latest, so a
later rerun may resolve a different source ID. Compare IDs from available failing runs and record
the ID in an issue when exact upstream identity matters; the suite does not promise that the
registry will retain an older version for reproduction.

## Reproduce a narrow case

Use a config ID from `tests/e2e/cases/`, and run from the repository root:

```sh
E2E_CASE=python/pyright make test-real-server-smoke-e2e
```

If the failure involves direct or detached process behavior, run the combined query and lifecycle
target with the same selector:

```sh
E2E_CASE=java/jdtls make test-real-server-e2e
```

For installation failures, isolate provisioning before starting the server:

```sh
E2E_SERVER=pyright make test-server-provisioning-e2e
```

These commands download external tools and require the host programs declared by the manifest.
They also use the current Mason registry; compare the reproduced source ID with the original before
concluding that behavior changed locally.

The manual **End-to-end compatibility** workflow can select `language`, `server`, or
`installation-family`. The value is respectively a case language ID, an LSP config ID, or one of:

```text
cargo generic github golang npm nuget pypi
```

For example, with the GitHub CLI:

```sh
gh workflow run e2e.yml -f selector=server -f value=pyright
```

Use `all` with an empty value for the complete matrix. The generated workflow summary shows all
selected pairs, including reviewed exclusions, before runnable pairs are sharded.

## Classify the failure

Check evidence in this order:

1. **Planner or manifest:** an unknown selector, missing registry package, or validation failure
   happened before a server case ran.
2. **Provisioning or network:** no completed receipt, package-manager output, HTTP failure, or an
   absent host program points to installation rather than LSP behavior. Use the provisioning-only
   command above.
3. **Startup or shutdown:** use the retained command line and server stderr. SDK incompatibility,
   launcher failure, crash, and failure to exit are distinct from query-result drift.
4. **Protocol or capability:** compare the retained capabilities with the command exercised by the
   case. An unadvertised optional capability passes only when lsp-cli returns its expected
   user-facing unsupported error.
5. **Semantic result:** compare stable names and the pair's reviewed exceptions. Do not weaken an
   expectation until the same source ID reproduces the behavior or an upstream change is confirmed.
6. **Cleanup:** inspect sandbox and runtime roots independently of the primary failure. A successful
   query with a retained root is still a cleanup regression.

Classify the result as an lsp-cli regression, fixture/manifest drift, provisioning or network
failure, upstream behavior change, known server limitation, or expected unsupported capability.
Do not add unbounded retries or turn crashes and provisioning failures into accepted query
exceptions. A confirmed stable server limitation belongs in both its manifest disposition and
`GOTCHAS.md`; an lsp-cli defect needs a focused regression test.
