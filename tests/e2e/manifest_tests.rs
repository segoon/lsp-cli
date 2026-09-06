use super::*;
use crate::repository_root;
use std::fs;

fn first_smoke(manifest: &mut Manifest) -> &mut SmokeDisposition {
    manifest
        .pairs
        .iter_mut()
        .find(|pair| pair.smoke.is_some())
        .expect("manifest should contain a smoke pair")
        .smoke
        .as_mut()
        .expect("selected pair should have a smoke case")
}

#[test]
fn partial_manifest_matches_pinned_data() {
    Manifest::load()
        .expect("E2E manifest should parse")
        .validate(repository_root())
        .expect("E2E manifest should be valid");
}

#[test]
fn copied_projects_do_not_contain_checkout_bound_compilation_databases() {
    let manifest = Manifest::load().expect("E2E manifest should parse");
    let repository = repository_root();

    for language in &manifest.languages {
        let project = repository.join(&language.project);
        let databases = files_named(&project, "compile_commands.json");
        assert!(
            databases.is_empty(),
            "E2E project {} contains compilation databases that cannot remain valid when copied: {databases:?}",
            language.project.display()
        );
    }
}

fn files_named(directory: &Path, expected: &str) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.expect("project directory entry should be readable");
        let file_type = entry
            .file_type()
            .expect("project directory entry type should be readable");
        let path = entry.path();
        if file_type.is_dir() {
            matches.extend(files_named(&path, expected));
        } else if path.file_name().and_then(|name| name.to_str()) == Some(expected) {
            matches.push(path);
        }
    }
    matches
}

#[test]
fn source_languages_select_the_approved_preferred_servers() {
    let manifest = Manifest::load().expect("E2E manifest should parse");
    let expected = [
        ("c", "clangd"),
        ("cpp", "clangd"),
        ("cs", "roslyn_ls"),
        ("cuda", "clangd"),
        ("go", "gopls"),
        ("java", "jdtls"),
        ("javascript", "ts_ls"),
        ("kotlin", "kotlin_lsp"),
        ("lua", "lua_ls"),
        ("objc", "clangd"),
        ("objcpp", "clangd"),
        ("python", "pyright"),
        ("rust", "rust_analyzer"),
        ("typescript", "ts_ls"),
    ];

    let languages = expected
        .iter()
        .map(|(language, _server)| (*language).to_string())
        .collect();
    let preferred = preferred_pairs(&repository_root().join("data"), &languages)
        .expect("data preferences should resolve");
    let actual = preferred
        .iter()
        .map(|pair| (pair.language.as_str(), pair.server.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected.as_slice());

    manifest
        .validate(repository_root())
        .expect("preferred data pairs should be declared");
}

#[test]
fn complete_mode_rejects_the_partial_matrix() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest.coverage = Coverage::Complete;

    let error = manifest
        .validate(repository_root())
        .expect_err("partial matrix should not satisfy complete coverage");

    assert!(error.contains("complete E2E manifest is missing pairs"));
}

#[test]
fn complete_mode_rejects_missing_server_pairs() {
    let data = repository_root().join("data");
    let detectable = detectable_languages(&data).expect("filetype configs should load");
    let declared_pairs = BTreeSet::from([PairKey {
        language: "rust".to_string(),
        server: "rust_analyzer".to_string(),
    }]);

    let error = Manifest::validate_complete_coverage(&data, &detectable, &declared_pairs)
        .expect_err("partial server matrix should not satisfy complete coverage");

    assert!(error.contains("complete E2E manifest is missing pairs"));
}

#[test]
fn manifest_rejects_unknown_fields() {
    let error = serde_yaml::from_str::<SuiteFile>(
        "schema-version: 4\ncoverage: partial\ncommands: []\nunknown: true\n",
    )
    .expect_err("unknown manifest fields should fail");

    assert!(error.to_string().contains("unknown field `unknown`"));
}

#[test]
fn manifest_rejects_a_source_language_without_a_preferred_server() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest
        .pairs
        .retain(|pair| !(pair.language == "c" && pair.server == "clangd"));

    let error = manifest
        .validate(repository_root())
        .expect_err("source language should require one preferred server");

    assert!(error.contains("missing its data-preferred server pair"));
}

#[test]
fn language_case_filename_must_match_its_id() {
    let case = serde_yaml::from_str::<LanguageFile>(
        "language:\n  id: gomod\n  kind: metadata\n  project: playground/gomod\n",
    )
    .expect("language case should parse");

    let error = case
        .into_parts("wrong", Path::new("cases/wrong.yaml"))
        .expect_err("case filename should match language ID");

    assert!(error.contains("does not match language ID"));
}

#[test]
fn language_case_must_own_its_pairs() {
    let case = serde_yaml::from_str::<LanguageFile>(
        "language:\n  id: gomod\n  kind: metadata\n  project: playground/gomod\npairs:\n  - language: gowork\n    server: gopls\n",
    )
    .expect("language case should parse");

    let error = case
        .into_parts("gomod", Path::new("cases/gomod.yaml"))
        .expect_err("case should contain only its own language pairs");

    assert!(error.contains("contains pair for language"));
}

#[test]
fn manifest_rejects_duplicate_command_coverage() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let duplicate = manifest
        .commands
        .first()
        .expect("manifest should contain command coverage")
        .clone();
    manifest.commands.push(duplicate);

    let error = manifest
        .validate(repository_root())
        .expect_err("duplicate command coverage should fail");

    assert!(error.contains("more than once"));
}

#[test]
fn manifest_rejects_config_path_traversal() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest
        .languages
        .first_mut()
        .expect("manifest should contain a language")
        .id = "../rust".to_string();
    manifest
        .pairs
        .first_mut()
        .expect("manifest should contain a pair")
        .language = "../rust".to_string();

    let error = manifest
        .validate(repository_root())
        .expect_err("config path traversal should fail");

    assert!(error.contains("must be one normalized path component"));
}

#[test]
fn manifest_rejects_invalid_smoke_deadlines() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let smoke = first_smoke(&mut manifest);
    let SmokeDisposition::Queries {
        lsp_timeout_seconds,
        deadline_seconds,
        ..
    } = smoke
    else {
        panic!("first smoke case should contain queries")
    };
    *deadline_seconds = lsp_timeout_seconds.saturating_sub(1);

    let error = manifest
        .validate(repository_root())
        .expect_err("short overall deadline should fail");

    assert!(error.contains("deadlines must be positive and ordered"));
}

#[test]
fn manifest_rejects_duplicate_host_programs() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let setup = manifest
        .pairs
        .iter_mut()
        .filter_map(|pair| pair.setup.as_mut())
        .find(|setup| !setup.host_programs.is_empty())
        .expect("manifest should contain a host-dependent query case");
    let duplicate = setup
        .host_programs
        .first()
        .expect("setup should have a host program")
        .clone();
    setup.host_programs.push(duplicate);

    let error = manifest
        .validate(repository_root())
        .expect_err("duplicate host programs should fail");

    assert!(error.contains("invalid host program"));
}

#[test]
fn manifest_rejects_a_preferred_pair_without_a_disposition() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest
        .pairs
        .iter_mut()
        .find(|pair| pair.language == "rust")
        .expect("Rust pair should exist")
        .smoke = None;

    let error = manifest
        .validate(repository_root())
        .expect_err("preferred pair should require a disposition");

    assert!(error.contains("must declare queries or an exclusion"));
}

#[test]
fn manifest_rejects_an_empty_exclusion_reason() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let pair = manifest
        .pairs
        .iter_mut()
        .find(|pair| matches!(pair.smoke, Some(SmokeDisposition::Excluded { .. })))
        .expect("manifest should contain an exclusion");
    pair.smoke = Some(SmokeDisposition::Excluded {
        reason: " ".to_string(),
    });

    let error = manifest
        .validate(repository_root())
        .expect_err("empty exclusions should fail");

    assert!(error.contains("must be non-empty"));
}

#[test]
fn every_preferred_server_requires_one_lifecycle_owner() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest
        .pairs
        .iter_mut()
        .find(|pair| pair.server == "clangd" && pair.lifecycle.is_some())
        .expect("clangd lifecycle owner should exist")
        .lifecycle = None;

    let error = manifest
        .validate(repository_root())
        .expect_err("preferred server should require a lifecycle owner");

    assert!(error.contains("must have exactly one lifecycle owner; found 0"));
}

#[test]
fn preferred_server_rejects_duplicate_lifecycle_owners() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let lifecycle = manifest
        .pairs
        .iter()
        .find(|pair| pair.server == "clangd" && pair.lifecycle.is_some())
        .expect("clangd lifecycle owner should exist")
        .lifecycle
        .clone();
    manifest
        .pairs
        .iter_mut()
        .find(|pair| pair.server == "clangd" && pair.lifecycle.is_none())
        .expect("clangd should have another language pair")
        .lifecycle = lifecycle;

    let error = manifest
        .validate(repository_root())
        .expect_err("preferred server should reject duplicate lifecycle owners");

    assert!(error.contains("must have exactly one lifecycle owner; found 2"));
}

#[test]
fn executable_lifecycle_requires_server_setup() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let pair = manifest
        .pairs
        .iter_mut()
        .find(|pair| matches!(pair.lifecycle, Some(LifecycleDisposition::Scenarios { .. })))
        .expect("manifest should contain a lifecycle scenario");
    pair.setup = None;

    let error = manifest
        .validate(repository_root())
        .expect_err("lifecycle scenario should require setup");

    assert!(error.contains("must declare server setup"));
}

#[test]
fn direct_run_exclusion_requires_a_reason() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let pair = manifest
        .pairs
        .iter_mut()
        .find(|pair| pair.language == "java")
        .expect("Java pair should exist");
    let Some(LifecycleDisposition::Scenarios { direct_run, .. }) = &mut pair.lifecycle else {
        panic!("Java should have lifecycle scenarios")
    };
    *direct_run = lifecycle_case::DirectRunDisposition::Excluded {
        reason: " ".to_string(),
    };

    let error = manifest
        .validate(repository_root())
        .expect_err("direct run exclusion should require a reason");

    assert!(error.contains("direct-run exclusion") && error.contains("must be non-empty"));
}

#[test]
fn lifecycle_exclusion_requires_a_reason() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let pair = manifest
        .pairs
        .iter_mut()
        .find(|pair| pair.language == "cs")
        .expect("C# pair should exist");
    pair.lifecycle = Some(LifecycleDisposition::Excluded {
        reason: " ".to_string(),
    });

    let error = manifest
        .validate(repository_root())
        .expect_err("lifecycle exclusion should require a reason");

    assert!(error.contains("lifecycle exclusion") && error.contains("must be non-empty"));
}

#[test]
fn lifecycle_scenario_rejects_invalid_deadlines() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let lifecycle = manifest
        .pairs
        .iter_mut()
        .find_map(|pair| pair.lifecycle.as_mut())
        .expect("manifest should contain lifecycle coverage");
    let LifecycleDisposition::Scenarios {
        lsp_timeout_seconds,
        deadline_seconds,
        ..
    } = lifecycle
    else {
        panic!("first lifecycle case should contain scenarios")
    };
    *deadline_seconds = lsp_timeout_seconds.saturating_sub(1);

    let error = manifest
        .validate(repository_root())
        .expect_err("lifecycle scenario should reject invalid deadlines");

    assert!(error.contains("lifecycle case") && error.contains("deadlines"));
}

#[test]
fn manifest_rejects_a_failure_exception_without_a_message() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let exception = manifest
        .pairs
        .iter_mut()
        .filter_map(|pair| pair.smoke.as_mut())
        .find_map(|smoke| match smoke {
            SmokeDisposition::Queries { exceptions, .. } => exceptions
                .iter_mut()
                .find(|item| item.outcome == ExceptionOutcome::Failure),
            SmokeDisposition::Excluded { .. } => None,
        })
        .expect("manifest should contain an expected failure");
    exception.message = None;

    let error = manifest
        .validate(repository_root())
        .expect_err("expected failures should require stable diagnostics");

    assert!(error.contains("must declare a message"));
}
