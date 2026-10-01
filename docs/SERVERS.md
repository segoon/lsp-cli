# Real-server coverage policy

This document records which catalog servers are intentionally outside automatic semantic E2E
coverage. The inventories reflect `tests/e2e/cases/suite.yaml` and the pinned Mason investigation
recorded in `PLAN.md`; registry changes can make them stale.

## Specialized servers: capability-only

The following 53 linters, formatters, framework servers, adapters, and authenticated services have
capability-only coverage. They do not need to satisfy the shared semantic-query profile. A case
still requires a runnable package, a suitable fixture, and any required credentials or host tools.

`angularls`, `ast_grep`, `azure_pipelines_ls`, `bacon_ls`, `codebook`, `cssmodules_ls`, `debputy`,
`djls`, `djlsp`, `dprint`, `efm`, `ember`, `emmet_language_server`, `emmet_ls`, `eslint`,
`fortitude`, `gh_actions_ls`, `gitlab_duo`, `golangci_lint_ls`, `grammarly`, `harper_ls`, `herb_ls`,
`htmx`, `hydra_lsp`, `kakehashi`, `laravel_ls`, `ltex`, `ltex_plus`, `lwc_ls`,
`nextflow_ls`, `quick_lint_js`, `ruff`, `selene3p_ls`, `shopify_theme_ls`, `slint_lsp`, `snyk_ls`,
`spectral`, `sqruff`, `statix`, `stimulus_ls`, `stylelint_lsp`, `stylua`,
`stylua3p_ls`, `tailwindcss`, `tflint`, `theme_check`, `ttags`, `turbo_ls`, `tvm_ffi_navigator`,
`unocss`, `vacuum`, `vale_ls`, `wc_language_server`.

Capability-only is a coverage classification, not a promise that lsp-cli can currently install or
initialize every server in this list. Authenticated services remain unusable without credentials.

## Unsupported installer families: excluded

lsp-cli will not add installer support for these 13 Mason packages yet. They remain excluded:

| Installation mechanism | Servers |
| --- | --- |
| RubyGems | `rubocop`, `solargraph`, `sorbet`, `standardrb`, `steep` |
| LuaRocks | `digestif`, `fennel_ls`, `teal_ls` |
| Open VSX | `home_assistant`, `motoko_lsp` |
| OPAM | `ocamllsp` |
| Composer | `psalm` |
| Source build | `java_language_server` |

Adding a backend later requires an explicit design for executable resolution, runtime discovery,
cache identity, receipts, archive/path safety, and hermetic tests. See `docs/TODO.md`.

## Absent from Mason: not installable

The current Mason registry has no package for these 90 catalog servers. lsp-cli therefore treats
them as not automatically installable; it will not silently fall back to another registry or to a
system binary.

`ada_ls`, `agda_ls`, `alloy_ls`, `anakin_language_server`, `atlas`, `atopile`, `ballerina`,
`bitbake_language_server`, `blueprint_ls`, `brioche`, `buck2`, `buddy_ls`, `ccls`,
`cir_lsp_server`, `coffeesense`, `daedalus_ls`, `dafny`, `dartls`, `dcmls`, `dolmenls`, `dts_lsp`,
`ecsact`, `flow`, `fsharp_language_server`, `fstar`, `futhark_lsp`, `gdshader_lsp`, `ghcide`,
`ghdl_ls`, `glasgow`, `gleam`, `gnls`, `guile_ls`, `hhvm`, `hie`, `hlasm`, `idris2_lsp`,
`janet_lsp`, `koka`, `kulala_ls`, `lean3ls`, `m68k`, `metals`, `mint`, `mlir_lsp_server`,
`mlir_pdll_lsp_server`, `mojo`, `msbuild_project_tools_server`, `muon`, `nelua_lsp`, `nixd`,
`nushell`, `openscad_ls`, `oso`, `pact_ls`, `pasls`, `perlls`, `perlpls`, `phan`, `phptools`,
`please`, `pli`, `pony_lsp`, `poryscript_pls`, `prolog_ls`, `pug`, `racket_langserver`,
`rune_languageserver`, `scheme_langserver`, `scry`, `sixtyfps`, `smarty_ls`, `swift_mesonls`,
`syntax_tree`, `tblgen_lsp_server`, `terraform_lsp`, `tilt_ls`, `turtle_ls`, `typeprof`,
`typst_lsp`, `uiua`, `ungrammar_languageserver`, `unison`, `uvls`, `veridian`, `vsrocq`,
`yang_lsp`, `yls`, `ziggy`, `ziggy_schema`.

Users can still select an already installed server with `--no-download` when its configured command
is available. That does not make the server part of automatic provisioning coverage.

## Index completion: best-effort servers

These 15 servers exposed no usable terminal background-index signal in the real-server suite:

`basedpyright`, `clangd`, `clojure_lsp`, `jedi_language_server`, `lua_ls`, `luau_lsp`, `ols`,
`omnisharp`, `perlnavigator`, `pyright`, `roslyn_ls`, `ts_ls`, `vtsls`, `zls`, `zuban`.

For these servers, `build-index` uses explicit, data-driven best-effort semantics.
Success must mean that the bounded best-effort operation completed, not that lsp-cli confirmed a
fully indexed workspace. Servers outside this list retain confirmed-completion semantics. The
implementation does not infer best-effort mode from language names, timeout messages, or a
server-specific production branch.
