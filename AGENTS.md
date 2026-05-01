# Mission

lsp-cli is a high-level commandline tool that makes it possible to query LSP server from command line.


# Limitations

* Do not add new external dependencies without explicit user permission.
* Be careful when wording user output. Error message must describe what's wrong
  from the user point of view instead of dumping low-level integer error codes
  (unless the error reason is unknown).
* When editing files / proposes changes, always:
  - explicitly state pros/cons of the proposal
  - inform about made decisions and tricky/risky/controversial/ugly details
  - summarize alternatives with pros/cons
  - inform about architectural/strategic consequencies and possible future problems/limitations
* When done, inform the user about difficulties you've met during the work


# Commands

Test the code with the following commands:

```sh
cargo test -q
cargo clippy
```
