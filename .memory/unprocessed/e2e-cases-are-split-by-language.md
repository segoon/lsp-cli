# E2E case manifests are split by language

The E2E manifest is a directory rather than one `cases.yaml` file. Global command coverage lives
in `cases/suite.yaml`; each `cases/<language>.yaml` owns exactly one project and all server pairs
for that language. The `gowork` case intentionally contains only an isolated `go.work` file so its
detection does not overlap `gomod` or `go`.
