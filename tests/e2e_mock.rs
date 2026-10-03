use std::path::Path;

use lsp_cli_e2e_support::mock;

// Cargo supplies the binary path to this integration target, independently of its build layout.
macro_rules! mock_cases {
    ($($case:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case() {
                mock::run(Path::new(env!("CARGO_BIN_EXE_lsp-cli")), mock::$case);
            }
        )+
    };
}

mock_cases!(
    catalog_command_paths_are_covered,
    filesystem_command_paths_are_covered,
    lifecycle_command_paths_are_covered,
    run_forwards_a_complete_lsp_exchange,
    lsp_fixture_command_paths_are_covered,
    unadvertised_capabilities_produce_user_facing_errors,
    update_command_path_uses_local_release_fixture,
);

#[test]
fn mock_cases_find_cli_after_test_executable_is_relocated() {
    let directory = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .expect("relocated test directory should initialize");
    let executable = directory.path().join("relocated-tests");
    std::fs::copy(
        std::env::current_exe().expect("integration-test executable should be available"),
        &executable,
    )
    .expect("integration-test executable should copy");
    let output = std::process::Command::new(&executable)
        .args([
            "catalog_command_paths_are_covered",
            "--exact",
            "--nocapture",
        ])
        .current_dir(directory.path())
        .output()
        .expect("relocated test should start");
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed;"),
        "relocated mock test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
