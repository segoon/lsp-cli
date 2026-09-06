# TypeScript workspace symbols can fail before project activation

typescript-language-server 4.4.0 with TypeScript 6.0.3 advertised workspace symbols but returned
`No Project` when `workspace/symbol` was the first semantic request in a fresh session. This is
different from an unsupported capability and requires an explicit reviewed test outcome.
