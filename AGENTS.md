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
  - explicitly retell questions the user was asked by you with the user answers
* When done, inform the user about difficulties you've met during the work.
  If you had no difficulties, omit the report.
* If you meet any difficulties with LSP protocol or LSP server implementation
  (e.g. bugs or non-standard API), write it down into `GOTCHAS.md`
  to the relevant section.
* Note that you're a consultant, not a product owner.
  Only the user may make important architectural desicions.
  If you have any ideas, remarks, suggestions, or you see extra problems with the user choise,
  you have to inform the user.


# Documentation

* `README.md` contains information easily readable by newbies who don't know anything about
  lsp-cli internals.
* If you implement some hack/tricky/complex code, write a short summary in the code comment
  why you do it this way.


# Testing

When adding/changing any major feature (e.g. subcommand),
check it:
- in unit tests in *.rs
- in playground/ (see @playground/README.md)

If you catch a bug in the code, write a regression test for that.

After you add/edit a test, check the whole file for code duplication in tests.
Don't leave similar boilerplate in tests.

# Commands

Test the code with the following commands:

```sh
cargo test -q
cargo clippy
```
