use super::*;
use crate::repository_root;

fn first_smoke(manifest: &mut Manifest) -> &mut SmokeCase {
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
    smoke.deadline_seconds = smoke.lsp_timeout_seconds.saturating_sub(1);

    let error = manifest
        .validate(repository_root())
        .expect_err("short overall deadline should fail");

    assert!(error.contains("deadline must not be shorter"));
}

#[test]
fn manifest_rejects_duplicate_host_programs() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    let smoke = first_smoke(&mut manifest);
    let duplicate = smoke
        .host_programs
        .first()
        .expect("smoke case should have a host program")
        .clone();
    smoke.host_programs.push(duplicate);

    let error = manifest
        .validate(repository_root())
        .expect_err("duplicate host programs should fail");

    assert!(error.contains("more than once"));
}
