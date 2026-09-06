# Direct run provisioning uses the overall case deadline

`lsp-cli run --download` replaces the CLI only after provisioning completes. A black-box protocol
test therefore cannot apply the shorter LSP request timeout to the entire child lifetime, or a
clean-home download may consume the budget before the server reads `initialize`. Direct-run tests
use the remaining case deadline; detached query commands still pass the request timeout to lsp-cli.
