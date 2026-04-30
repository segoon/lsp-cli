# Mission

lsp-cli is a high-level commandline tool that makes it possible to query LSP server from command line.


# Limitations

* Do not add new external dependencies without explicit user permission.
* Be careful when wording user output. Error message must describe what's wrong
  from the user point of view instead of dumping low-level integer error codes
  (unless the error reason is unknown).



# Commands

Test the code with the following commands:

```sh
cargo test -q
cargo clippy
```
