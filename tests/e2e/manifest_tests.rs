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
fn complete_manifest_matches_pinned_data() {
    let manifest = Manifest::load().expect("E2E manifest should parse");
    let data = repository_root().join("data");
    let detectable = detectable_languages(&data).expect("filetype configs should load");
    let compatible = manifest
        .compatible_pair_inventory(&data)
        .expect("manifest inventory should resolve");
    let declared = manifest
        .pairs
        .iter()
        .map(PairCase::key)
        .collect::<BTreeSet<_>>();
    let servers = compatible
        .iter()
        .map(|pair| pair.server.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(manifest.coverage, Coverage::Complete);
    assert_eq!(detectable.len(), 16);
    assert_eq!(servers.len(), 57);
    assert_eq!(compatible.len(), 141);
    assert_eq!(declared.len(), 31);
    assert_eq!(compatible.difference(&declared).count(), 110);
    assert_eq!(manifest.servers.len(), 57);
    assert_eq!(
        manifest
            .servers
            .iter()
            .filter(|server| server.is_downloadable())
            .count(),
        22
    );
    assert_eq!(
        manifest
            .servers
            .iter()
            .map(|server| server.id.as_str())
            .collect::<BTreeSet<_>>(),
        servers
    );
    assert!(declared.is_subset(&compatible));
    assert_eq!(manifest.platform_label(), "linux/x86_64");
    assert!(
        manifest
            .pairs
            .iter()
            .all(|pair| pair.smoke.is_some() || pair.lifecycle.is_some())
    );
    manifest
        .validate(repository_root())
        .expect("complete E2E manifest should be valid");
}

#[test]
fn complete_manifest_rejects_a_missing_downloadable_pair() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest
        .pairs
        .retain(|pair| !(pair.language == "python" && pair.server == "basedpyright"));

    let error = manifest
        .validate(repository_root())
        .expect_err("downloadable compatibility must have reviewed behavior");

    assert!(error.contains("missing downloadable pairs"));
    assert!(error.contains("python/basedpyright"));
}

#[test]
fn pair_selection_reports_explicit_and_inherited_exclusions() {
    let manifest = Manifest::load_validated(repository_root()).expect("manifest should validate");
    for pair in ["python/basedpyright", "c/ast_grep"] {
        assert!(manifest.declares_pair(pair));
        assert!(manifest.exclusion_reason(pair).is_some());
    }
}

#[test]
fn provisioning_inventory_rejects_missing_and_duplicate_servers() {
    let mut missing = Manifest::load().expect("E2E manifest should parse");
    let duplicate = missing
        .servers
        .first()
        .expect("manifest should contain provisioning servers")
        .clone();
    missing.servers.remove(0);
    let error = missing
        .validate(repository_root())
        .expect_err("missing provisioning server should fail");
    assert!(error.contains("does not match compatible servers"));

    let mut duplicated = Manifest::load().expect("E2E manifest should parse");
    duplicated.servers.push(duplicate);
    let error = duplicated
        .validate(repository_root())
        .expect_err("duplicate provisioning server should fail");
    assert!(error.contains("more than once"));
}

#[test]
fn provisioning_inventory_validates_dispositions_and_owners() {
    assert_invalid_provisioning(
        |server| {
            server.provisioning = ProvisioningDisposition::Excluded {
                reason: " ".to_string(),
            };
        },
        "must be non-empty",
    );
    assert_invalid_provisioning(
        |server| {
            server.provisioning = ProvisioningDisposition::Download {
                host_programs: Vec::new(),
                deadline_seconds: Some(0),
            };
        },
        "deadline",
    );
    assert_invalid_provisioning(
        |server| server.owner_language = "gomod".to_string(),
        "not compatible with owner language",
    );
}

fn assert_invalid_provisioning(mutate: impl FnOnce(&mut ServerCase), expected_error: &str) {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let server = manifest
        .servers
        .iter_mut()
        .find(|server| server.id == "clangd")
        .expect("clangd provisioning should exist");
    mutate(server);
    let error = manifest
        .validate(repository_root())
        .expect_err("invalid provisioning inventory should fail");
    assert!(error.contains(expected_error), "unexpected error: {error}");
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
fn complete_mode_rejects_a_missing_language_project() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let metadata_index = manifest
        .languages
        .iter()
        .position(|language| language.kind == ProjectKind::Metadata)
        .expect("complete manifest should contain a metadata project");
    let removed = manifest.languages.remove(metadata_index);
    manifest.pairs.retain(|pair| pair.language != removed.id);

    let error = manifest
        .validate(repository_root())
        .expect_err("missing project should not satisfy complete coverage");

    assert!(error.contains("complete E2E manifest is missing languages"));
    assert!(error.contains(&removed.id));
}

#[test]
fn manifest_rejects_a_pair_without_test_behavior() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let pair = manifest
        .pairs
        .first_mut()
        .expect("manifest should contain a configured case");
    pair.smoke = None;
    pair.lifecycle = None;

    let error = manifest
        .validate(repository_root())
        .expect_err("bare compatibility entries should stay in data");

    assert!(error.contains("must declare a smoke or lifecycle disposition"));
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
    *lsp_timeout_seconds = Some(10);
    *deadline_seconds = Some(9);

    let error = manifest
        .validate(repository_root())
        .expect_err("short overall deadline should fail");

    assert!(error.contains("deadlines must be positive and ordered"));
}

#[test]
fn manifest_rejects_duplicate_host_programs() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let programs = manifest
        .servers
        .iter_mut()
        .find_map(|server| match &mut server.provisioning {
            ProvisioningDisposition::Download { host_programs, .. }
                if !host_programs.is_empty() =>
            {
                Some(host_programs)
            }
            ProvisioningDisposition::Download { .. } | ProvisioningDisposition::Excluded { .. } => {
                None
            }
        })
        .expect("manifest should contain a host-dependent provisioning case");
    let duplicate = programs
        .first()
        .expect("provisioning should have a host program")
        .clone();
    programs.push(duplicate);

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
fn executable_lifecycle_requires_downloadable_server() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let server_id = manifest
        .pairs
        .iter()
        .find(|pair| matches!(pair.lifecycle, Some(LifecycleDisposition::Scenarios { .. })))
        .expect("manifest should contain a lifecycle scenario")
        .server
        .clone();
    manifest
        .servers
        .iter_mut()
        .find(|server| server.id == server_id)
        .expect("lifecycle server should have provisioning")
        .provisioning = ProvisioningDisposition::Excluded {
        reason: "test exclusion".to_string(),
    };

    let error = manifest
        .validate(repository_root())
        .expect_err("lifecycle scenario should require downloadable provisioning");

    assert!(error.contains("uses an excluded provisioning server"));
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
    *lsp_timeout_seconds = Some(10);
    *deadline_seconds = Some(9);

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
            SmokeDisposition::Capabilities { .. } | SmokeDisposition::Excluded { .. } => None,
        })
        .expect("manifest should contain an expected failure");
    exception.message = None;

    let error = manifest
        .validate(repository_root())
        .expect_err("expected failures should require stable diagnostics");

    assert!(error.contains("must declare a message"));
}
