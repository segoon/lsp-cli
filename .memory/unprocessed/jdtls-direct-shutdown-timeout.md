# jdtls can initialize but fail direct shutdown

The current Mason jdtls package initialized after Java and Python were staged, but a direct
`server-capabilities` command timed out waiting for the process to exit after shutdown. This makes
capability-driven direct query coverage unusable even though initialization itself succeeds.
