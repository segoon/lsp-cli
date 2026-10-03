fn main() {
    lsp_cli_e2e_support::run_reaper_regressions(std::path::Path::new(env!(
        "CARGO_BIN_EXE_lsp-cli"
    )));
}
