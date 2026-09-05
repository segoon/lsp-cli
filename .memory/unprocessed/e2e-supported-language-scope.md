# Configured and detectable language scopes differ

At pinned `lsp-cli-data` revision `013a75f6412917b710aa4683b9c5761c0c679975`, there are 362
filetype YAML files, but only 16 have a non-empty `extensions` or `patterns` list. The other 346
cannot be detected from a project even though the `languages` command currently collects every
loaded filetype ID without filtering for usable detection rules.

This makes “all supported languages” ambiguous for E2E coverage. The proposed executable scope is
the 16 detectable IDs and their 57 relevant LSP configurations (141 compatible pairs), while the
full catalog receives configuration-validation coverage. Product ownership must confirm whether
the `languages` command is intended to expose catalog entries or only detectable languages.
