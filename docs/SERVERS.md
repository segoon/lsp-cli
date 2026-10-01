# Real-server support matrix

The table has one row for every server in the validated E2E manifest. It is derived from
`tests/e2e/cases/suite.yaml`, its language-pair case files, and the pinned Mason investigation
recorded in `PLAN.md`. Registry and server releases can make it stale.

“Supported operations (E2E-covered)” describes real-server E2E coverage, not a promise that every
LSP method works for every project. “Semantic query profile” covers `server-capabilities`,
`diagnostics`, `format`,
`grep`, `list-symbols`, `list-functions`, `references`, `callers`, `callees`, `definition`,
`declaration`, `implementation`, `type-definition`, and `build-index`, subject to pair-specific
exceptions in the manifest. “Lifecycle” covers a direct initialize/shutdown exchange.

| Server | Mason installability | Supported operations (E2E-covered) | Notes |
| --- | --- | --- | --- |
| `ada_ls` | No — package absent | — | — |
| `agda_ls` | No — package absent | — | — |
| `aiken` | Yes | `server-capabilities` | — |
| `air` | Yes | `server-capabilities` | — |
| `alloy_ls` | No — package absent | — | — |
| `anakin_language_server` | No — package absent | — | — |
| `angularls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `antlersls` | Yes | `server-capabilities` | — |
| `arduino_language_server` | Yes | — | — |
| `asm_lsp` | Yes | `server-capabilities` | — |
| `ast_grep` | Yes | `server-capabilities` | 1 expected E2E failure |
| `astro` | Yes | — | — |
| `atlas` | No — package absent | — | — |
| `atopile` | No — package absent | — | — |
| `autohotkey_lsp` | Yes | `server-capabilities` | — |
| `autotools_ls` | Yes | `server-capabilities` | — |
| `awk_ls` | Yes | `server-capabilities` | — |
| `azure_pipelines_ls` | Yes | `server-capabilities` | — |
| `bacon_ls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `ballerina` | No — package absent | — | — |
| `basedpyright` | Yes | Semantic query profile | `build-index`: best effort |
| `bashls` | Yes | `server-capabilities` | — |
| `bazelrc_lsp` | Yes | `server-capabilities` | — |
| `beancount` | Yes | `server-capabilities` | — |
| `bicep` | Known failure | `server-capabilities` | 3 expected E2E failures |
| `bitbake_language_server` | No — package absent | — | — |
| `blueprint_ls` | No — package absent | — | — |
| `bqls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `bright_script` | Yes | `server-capabilities` | — |
| `brioche` | No — package absent | — | — |
| `buck2` | No — package absent | — | — |
| `buddy_ls` | No — package absent | — | — |
| `buf_ls` | Yes | `server-capabilities` | — |
| `bzl` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `c3_lsp` | Yes | `server-capabilities` | 2 expected E2E failures |
| `cairo_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `ccls` | No — package absent | — | — |
| `cds_lsp` | Yes | `server-capabilities` | — |
| `cir_lsp_server` | No — package absent | — | — |
| `circom-lsp` | Yes | `server-capabilities` | — |
| `clangd` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `clarinet` | Known failure | `server-capabilities` | 3 expected E2E failures |
| `clojure_lsp` | Yes | Semantic query profile; `server-capabilities`; Lifecycle | `build-index`: best effort |
| `cmake` | Yes | `server-capabilities` | — |
| `cobol_ls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `codebook` | Yes | `server-capabilities` | — |
| `coffeesense` | No — package absent | — | — |
| `crystalline` | Yes | `server-capabilities` | 1 expected E2E failure |
| `css_variables` | Yes | `server-capabilities` | — |
| `csskit` | Yes | `server-capabilities` | 1 expected E2E failure |
| `cssls` | Yes | `server-capabilities` | — |
| `cssmodules_ls` | Yes | `server-capabilities` | — |
| `cucumber_language_server` | Yes | `server-capabilities` | — |
| `cue` | Yes | `server-capabilities` | — |
| `cypher_ls` | Yes | `server-capabilities` | — |
| `daedalus_ls` | No — package absent | — | — |
| `dafny` | No — package absent | — | — |
| `dagger` | Yes | `server-capabilities` | — |
| `dartls` | No — package absent | — | — |
| `dcmls` | No — package absent | — | — |
| `debputy` | No — package absent | — | — |
| `denols` | Yes | `server-capabilities` | — |
| `dhall_lsp_server` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `digestif` | No — unsupported installer | — | — |
| `djls` | Yes | `server-capabilities` | — |
| `djlsp` | Yes | `server-capabilities` | — |
| `docker_compose_language_service` | Yes | `server-capabilities` | — |
| `docker_language_server` | Yes | `server-capabilities` | — |
| `dockerls` | Yes | `server-capabilities` | — |
| `dolmenls` | No — package absent | — | — |
| `dotls` | Yes | `server-capabilities` | — |
| `dprint` | Yes | `server-capabilities` | — |
| `dts_lsp` | No — package absent | — | — |
| `earthlyls` | Yes | `server-capabilities` | — |
| `ecsact` | No — package absent | — | — |
| `efm` | Yes | `server-capabilities` | 1 expected E2E failure |
| `elixirls` | Known failure | `server-capabilities` | 5 expected E2E failures |
| `elmls` | Yes | `server-capabilities` | — |
| `elp` | Yes | `server-capabilities` | — |
| `ember` | Yes | `server-capabilities` | — |
| `emmet_language_server` | Yes | `server-capabilities` | — |
| `emmet_ls` | Yes | `server-capabilities` | — |
| `emmylua_ls` | Yes | Semantic query profile | — |
| `erg_language_server` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `esbonio` | Yes | `server-capabilities` | — |
| `eslint` | Yes | `server-capabilities` | — |
| `expert` | Yes | `server-capabilities` | — |
| `facility_language_server` | Yes | `server-capabilities` | 1 expected E2E failure |
| `fennel_language_server` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `fennel_ls` | No — unsupported installer | — | — |
| `fish_lsp` | Yes | `server-capabilities` | 1 expected E2E failure |
| `flow` | No — package absent | — | — |
| `flux_lsp` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `foam_ls` | Yes | `server-capabilities` | 2 expected E2E failures |
| `fortitude` | No — package absent | — | — |
| `fortls` | Yes | `server-capabilities` | — |
| `fsautocomplete` | Yes | `server-capabilities` | — |
| `fsharp_language_server` | No — package absent | — | — |
| `fstar` | No — package absent | — | — |
| `futhark_lsp` | No — package absent | — | — |
| `gdshader_lsp` | No — package absent | — | — |
| `gh_actions_ls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `ghcide` | No — package absent | — | — |
| `ghdl_ls` | No — package absent | — | — |
| `ginko_ls` | Yes | `server-capabilities` | — |
| `gitlab_ci_ls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `gitlab_duo` | No — package absent | — | — |
| `glasgow` | No — package absent | — | — |
| `gleam` | No — package absent | — | — |
| `glsl_analyzer` | Yes | `server-capabilities` | — |
| `glslls` | Known failure | `server-capabilities` | 8 expected E2E failures |
| `gn_language_server` | Yes | `server-capabilities` | — |
| `gnls` | No — package absent | — | — |
| `golangci_lint_ls` | Yes | `server-capabilities` | — |
| `gopls` | Yes | Semantic query profile; `server-capabilities`; Lifecycle | — |
| `grammarly` | Yes | `server-capabilities` | 1 expected E2E failure |
| `graphql` | Yes | `server-capabilities` | — |
| `groovyls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `guile_ls` | No — package absent | — | — |
| `harper_ls` | Yes | `server-capabilities` | — |
| `hdl_checker` | Yes | `server-capabilities` | — |
| `helm_ls` | Yes | `server-capabilities` | 2 expected E2E failures |
| `herb_ls` | Yes | `server-capabilities` | — |
| `hhvm` | No — package absent | — | — |
| `hie` | No — package absent | — | — |
| `hlasm` | No — package absent | — | — |
| `hls` | Known failure | `server-capabilities` | 4 expected E2E failures |
| `home_assistant` | No — unsupported installer | — | — |
| `hoon_ls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `html` | Yes | `server-capabilities` | — |
| `htmx` | Yes | `server-capabilities` | 1 expected E2E failure |
| `hydra_lsp` | Yes | `server-capabilities` | — |
| `hylo_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `hyprls` | Yes | `server-capabilities` | — |
| `idris2_lsp` | No — package absent | — | — |
| `intelephense` | Yes | `server-capabilities` | — |
| `janet_lsp` | No — package absent | — | — |
| `java_language_server` | No — package absent | — | — |
| `jdtls` | Yes | Lifecycle | — |
| `jedi_language_server` | Yes | Semantic query profile | `build-index`: best effort |
| `jinja_lsp` | Yes | `server-capabilities` | — |
| `jqls` | Yes | `server-capabilities` | — |
| `jsonls` | Yes | `server-capabilities` | — |
| `jsonnet_ls` | Yes | `server-capabilities` | — |
| `julials` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `just` | Yes | `server-capabilities` | — |
| `kakehashi` | Yes | `server-capabilities` | — |
| `kcl` | Yes | `server-capabilities` | — |
| `koka` | No — package absent | — | — |
| `kotlin_language_server` | Yes | — | — |
| `kotlin_lsp` | Yes | Lifecycle | — |
| `kulala_ls` | No — package absent | — | — |
| `laravel_ls` | Yes | `server-capabilities` | — |
| `lean3ls` | No — package absent | — | — |
| `lelwel_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `lemminx` | Yes | `server-capabilities` | — |
| `lexical` | Yes | `server-capabilities` | 4 expected E2E failures |
| `ltex` | Yes | `server-capabilities` | — |
| `ltex_plus` | Yes | `server-capabilities` | — |
| `lua_ls` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `luau_lsp` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `lwc_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `m68k` | No — package absent | — | — |
| `markdown_oxide` | Yes | `server-capabilities` | 1 expected E2E failure |
| `marko-js` | Yes | `server-capabilities` | — |
| `marksman` | Yes | `server-capabilities` | — |
| `matlab_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `mdx_analyzer` | Yes | `server-capabilities` | 1 expected E2E failure |
| `mesonlsp` | Yes | `server-capabilities` | 1 expected E2E failure |
| `metals` | No — package absent | — | — |
| `millet` | Yes | `server-capabilities` | — |
| `mint` | No — package absent | — | — |
| `mlir_lsp_server` | No — package absent | — | — |
| `mlir_pdll_lsp_server` | No — package absent | — | — |
| `mm0_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `mojo` | No — package absent | — | — |
| `motoko_lsp` | No — unsupported installer | — | — |
| `move_analyzer` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `mpls` | Yes | `server-capabilities` | — |
| `msbuild_project_tools_server` | No — package absent | — | — |
| `muon` | No — package absent | — | — |
| `mutt_ls` | Yes | `server-capabilities` | — |
| `nelua_lsp` | No — package absent | — | — |
| `neocmake` | Yes | `server-capabilities` | 1 expected E2E failure |
| `nextflow_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `nextls` | Yes | `server-capabilities` | 4 expected E2E failures |
| `nginx_language_server` | Yes | `server-capabilities` | — |
| `nickel_ls` | Yes | `server-capabilities` | — |
| `nil_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `nim_langserver` | Yes | `server-capabilities` | — |
| `nimls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `nixd` | No — package absent | — | — |
| `ntt` | Yes | `server-capabilities` | 1 expected E2E failure |
| `nushell` | No — package absent | — | — |
| `nxls` | Yes | `server-capabilities` | — |
| `ocamllsp` | No — unsupported installer | — | — |
| `ols` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `omnisharp` | Yes | Semantic query profile; `server-capabilities`; Lifecycle | `build-index`: best effort |
| `opencl_ls` | Yes | `server-capabilities` | — |
| `openscad_ls` | No — package absent | — | — |
| `openscad_lsp` | Yes | `server-capabilities` | — |
| `oso` | No — package absent | — | — |
| `pact_ls` | No — package absent | — | — |
| `pasls` | No — package absent | — | — |
| `pbls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `perlls` | No — package absent | — | — |
| `perlnavigator` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `perlpls` | No — package absent | — | — |
| `pest_ls` | Yes | `server-capabilities` | — |
| `phan` | No — package absent | — | — |
| `phpactor` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `phptools` | No — package absent | — | — |
| `pico8_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `please` | No — package absent | — | — |
| `pli` | No — package absent | — | — |
| `pony_lsp` | No — package absent | — | — |
| `poryscript_pls` | No — package absent | — | — |
| `postgres_lsp` | Yes | `server-capabilities` | — |
| `powershell_es` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `prismals` | Yes | `server-capabilities` | — |
| `prolog_ls` | No — package absent | — | — |
| `prosemd_lsp` | Yes | `server-capabilities` | — |
| `protols` | Yes | `server-capabilities` | — |
| `psalm` | No — unsupported installer | — | — |
| `pug` | No — package absent | — | — |
| `puppet` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `purescriptls` | Yes | `server-capabilities` | — |
| `pylsp` | Yes | — | — |
| `pylyzer` | Yes | Semantic query profile | 1 expected E2E failure |
| `pyre` | Yes | — | — |
| `pyrefly` | Yes | — | — |
| `pyright` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `qmlls` | Yes | `server-capabilities` | — |
| `quick_lint_js` | Yes | `server-capabilities` | — |
| `r_language_server` | Known failure | `server-capabilities` | 4 expected E2E failures |
| `racket_langserver` | No — package absent | — | — |
| `raku_navigator` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `reason_ls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `regal` | Yes | `server-capabilities` | — |
| `regols` | Yes | `server-capabilities` | 1 expected E2E failure |
| `remark_ls` | Yes | `server-capabilities` | — |
| `rescriptls` | Yes | `server-capabilities` | — |
| `rls` | No — package absent | — | — |
| `rnix` | Yes | `server-capabilities` | — |
| `robotcode` | Yes | `server-capabilities` | — |
| `robotframework_ls` | Yes | `server-capabilities` | — |
| `roc_ls` | Yes | `server-capabilities` | — |
| `rome` | No — package absent | — | — |
| `roslyn_ls` | Yes | Semantic query profile; Lifecycle | `build-index`: best effort |
| `rpmspec` | Yes | — | — |
| `rubocop` | No — unsupported installer | — | — |
| `ruff` | Yes | `server-capabilities` | — |
| `ruff_lsp` | No — package absent | — | — |
| `rumdl` | Yes | `server-capabilities` | — |
| `rune_languageserver` | No — package absent | — | — |
| `rust_analyzer` | Yes | Semantic query profile; Lifecycle | — |
| `salt_ls` | Known failure | — | 1 expected E2E failure |
| `scheme_langserver` | No — package absent | — | — |
| `scry` | No — package absent | — | — |
| `selene3p_ls` | No — package absent | — | — |
| `serve_d` | Yes | `server-capabilities` | — |
| `shopify_theme_ls` | Yes | `server-capabilities` | — |
| `sixtyfps` | No — package absent | — | — |
| `slangd` | Yes | `server-capabilities` | — |
| `slint_lsp` | Yes | `server-capabilities` | — |
| `smarty_ls` | No — package absent | — | — |
| `smithy_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `snakeskin_ls` | Yes | `server-capabilities` | — |
| `snyk_ls` | Yes | `server-capabilities` | — |
| `solang` | Yes | `server-capabilities` | — |
| `solargraph` | No — unsupported installer | — | — |
| `solc` | Yes | `server-capabilities` | 1 expected E2E failure |
| `solidity` | Yes | `server-capabilities` | — |
| `solidity_ls` | Yes | `server-capabilities` | — |
| `solidity_ls_nomicfoundation` | Yes | `server-capabilities` | — |
| `somesass_ls` | Yes | `server-capabilities` | — |
| `sorbet` | No — unsupported installer | — | — |
| `sourcekit` | No — package absent | — | — |
| `spectral` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `spyglassmc_language_server` | Yes | `server-capabilities` | — |
| `sqlls` | Yes | `server-capabilities` | 1 expected E2E failure |
| `sqls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `sqruff` | No — package absent | — | — |
| `stan_ls` | Yes | `server-capabilities` | — |
| `standardrb` | No — unsupported installer | — | — |
| `starlark_rust` | Yes | `server-capabilities` | — |
| `starpls` | Yes | `server-capabilities` | — |
| `statix` | No — package absent | — | — |
| `steep` | No — unsupported installer | — | — |
| `stimulus_ls` | Yes | `server-capabilities` | — |
| `stylelint_lsp` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `stylua` | Yes | `server-capabilities` | — |
| `stylua3p_ls` | No — package absent | — | — |
| `svelte` | Yes | `server-capabilities` | — |
| `svlangserver` | Yes | `server-capabilities` | — |
| `svls` | Yes | `server-capabilities` | — |
| `swift_mesonls` | No — package absent | — | — |
| `syntax_tree` | No — package absent | — | — |
| `systemd_lsp` | Yes | `server-capabilities` | — |
| `tailwindcss` | Yes | `server-capabilities` | — |
| `taplo` | Yes | `server-capabilities` | — |
| `tblgen_lsp_server` | No — package absent | — | — |
| `tclsp` | Yes | `server-capabilities` | — |
| `teal_ls` | No — unsupported installer | — | — |
| `templ` | Yes | `server-capabilities` | 1 expected E2E failure |
| `terraform_lsp` | No — package absent | — | — |
| `terraformls` | Yes | `server-capabilities` | — |
| `texlab` | Yes | `server-capabilities` | — |
| `textlsp` | Yes | `server-capabilities` | — |
| `tflint` | Yes | `server-capabilities` | — |
| `theme_check` | No — package absent | — | — |
| `thriftls` | Yes | `server-capabilities` | — |
| `tilt_ls` | No — package absent | — | — |
| `tinymist` | Yes | `server-capabilities` | — |
| `tofu_ls` | Yes | `server-capabilities` | — |
| `tombi` | Yes | `server-capabilities` | — |
| `ts_ls` | Yes | Semantic query profile; `server-capabilities`; Lifecycle | `build-index`: best effort |
| `ts_query_ls` | Yes | `server-capabilities` | — |
| `tsp_server` | Yes | `server-capabilities` | — |
| `ttags` | No — package absent | — | — |
| `turbo_ls` | No — package absent | — | — |
| `turtle_ls` | No — package absent | — | — |
| `tvm_ffi_navigator` | No — package absent | — | — |
| `twiggy_language_server` | Yes | `server-capabilities` | — |
| `ty` | Yes | — | — |
| `typeprof` | No — package absent | — | — |
| `typst_lsp` | No — package absent | — | — |
| `uiua` | No — package absent | — | — |
| `ungrammar_languageserver` | No — package absent | — | — |
| `unison` | No — package absent | — | — |
| `unocss` | Yes | `server-capabilities` | — |
| `uvls` | No — package absent | — | — |
| `v_analyzer` | Yes | `server-capabilities` | 3 expected E2E failures |
| `vacuum` | No — package absent | — | — |
| `vala_ls` | Known failure | `server-capabilities` | 3 expected E2E failures |
| `vale_ls` | Yes | `server-capabilities` | — |
| `verible` | Yes | `server-capabilities` | — |
| `veridian` | No — package absent | — | — |
| `veryl_ls` | Yes | `server-capabilities` | — |
| `vespa_ls` | Known failure | `server-capabilities` | 4 expected E2E failures |
| `vhdl_ls` | Known failure | `server-capabilities` | 3 expected E2E failures |
| `vimls` | Yes | `server-capabilities` | — |
| `visualforce_ls` | Known failure | `server-capabilities` | 2 expected E2E failures |
| `vls` | Known failure | `server-capabilities` | 3 expected E2E failures |
| `vsrocq` | No — package absent | — | — |
| `vtsls` | Yes | Semantic query profile; `server-capabilities` | `build-index`: best effort |
| `vue_ls` | Yes | `server-capabilities` | — |
| `wasm_language_tools` | Yes | `server-capabilities` | — |
| `wc_language_server` | Yes | `server-capabilities` | — |
| `wgsl_analyzer` | Yes | `server-capabilities` | — |
| `yamlls` | Yes | `server-capabilities` | — |
| `yang_lsp` | No — package absent | — | — |
| `yls` | No — package absent | — | — |
| `ziggy` | No — package absent | — | — |
| `ziggy_schema` | No — package absent | — | — |
| `zk` | Yes | `server-capabilities` | — |
| `zls` | Yes | Semantic query profile; `server-capabilities`; Lifecycle | `build-index`: best effort |
| `zuban` | Yes | Semantic query profile | `build-index`: best effort |

## Interpretation

- **Yes** means the pinned Mason registry has a package using an installer lsp-cli supports. It
  does not guarantee that every upstream release asset or server behavior is currently healthy.
- **Known failure** means Mason resolves a supported package, but the pinned E2E case records a
  provisioning failure such as a missing platform asset, broken URL, unresolved executable, or
  missing host runtime. See `docs/GOTCHAS.md` for details.
- **No — package absent** means the pinned Mason registry has no matching package. Users may still
  select an already installed server with `--no-download`; those operations are not automatically
  covered here.
- **No — unsupported installer** means Mason has a package, but its installer family is outside
  lsp-cli’s current product boundary. Adding a backend requires explicit executable resolution,
  runtime discovery, cache identity, receipt, archive/path-safety, and hermetic-test design. See
  `docs/TODO.md`.
- **`build-index`: best effort** means the server exposes no usable terminal background-index
  signal. lsp-cli observes the complete bounded wait and accepts expiry without claiming that the
  workspace is fully indexed. Other servers retain confirmed-completion semantics.
