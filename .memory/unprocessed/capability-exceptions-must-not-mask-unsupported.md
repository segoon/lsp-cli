# Known-result exceptions must not mask unsupported capabilities

The first capability runner applied manifest exceptions before checking the server's advertised
capability. A TypeScript call-hierarchy exception then expected `No Project` even though the server
did not advertise call hierarchy and lsp-cli correctly returned its unsupported-capability error.
Capability absence must take precedence; expected failure fragments also prevent unrelated
provisioning or crash errors from satisfying a reviewed exception.
