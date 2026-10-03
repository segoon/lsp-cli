fn main() -> std::process::ExitCode {
    lsp_cli_e2e_support::run_suite(std::path::Path::new(env!("CARGO_BIN_EXE_lsp-cli")))
}
