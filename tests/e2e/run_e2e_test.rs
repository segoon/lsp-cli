#[cfg(unix)]
mod tests {
    use crate::harness::E2eContext;
    use crate::repository_root;
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Output};

    fn run_wrapper(cargo_output: &str, cargo_status: u8) -> Output {
        let context = E2eContext::new().expect("E2E context should initialize");
        let bin_dir = context.workspace().join("bin");
        fs::create_dir(&bin_dir).expect("fake Cargo directory should be created");
        let cargo = bin_dir.join("cargo");
        fs::write(
            &cargo,
            "#!/bin/sh\nprintf '%s' \"$FAKE_CARGO_OUTPUT\"\nexit \"$FAKE_CARGO_STATUS\"\n",
        )
        .expect("fake Cargo should be written");
        let mut permissions = fs::metadata(&cargo)
            .expect("fake Cargo metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&cargo, permissions).expect("fake Cargo should be executable");

        let mut paths = vec![bin_dir];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
        Command::new(repository_root().join("scripts/run_e2e_test.sh"))
            .arg("manifest_real_server")
            .env("FAKE_CARGO_OUTPUT", cargo_output)
            .env("FAKE_CARGO_STATUS", cargo_status.to_string())
            .env(
                "PATH",
                env::join_paths(paths).expect("test PATH should be valid"),
            )
            .env("TMPDIR", context.workspace())
            .output()
            .expect("E2E test wrapper should run")
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8(output.stdout.clone()).expect("wrapper stdout should be UTF-8")
    }

    #[test]
    fn failure_footer_names_sorted_unique_case_ids_and_preserves_status() {
        let output = run_wrapper(
            "E2E lifecycle java/jdtls failed:\n\
             E2E case go/gopls failed:\n\
             E2E capabilities case java/jdtls failed:\n\
             E2E provisioning python/pyright failed:\n",
            17,
        );

        assert_eq!(output.status.code(), Some(17));
        assert!(
            stdout(&output)
                .ends_with("\nE2E failed case IDs:\n- go/gopls\n- java/jdtls\n- python/pyright\n")
        );
    }

    #[test]
    fn failure_footer_explains_when_case_id_is_unavailable() {
        let output = run_wrapper("cargo failed before a case started\n", 1);

        assert_eq!(output.status.code(), Some(1));
        assert!(
            stdout(&output).ends_with(
                "\nE2E failed case IDs:\n- unavailable; inspect the diagnostics above\n"
            )
        );
    }

    #[test]
    fn successful_run_has_no_failure_footer() {
        let output = run_wrapper("all tests passed\n", 0);

        assert!(output.status.success());
        assert_eq!(stdout(&output), "all tests passed\n");
    }
}
