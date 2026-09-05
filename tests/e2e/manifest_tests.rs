use super::*;
use crate::repository_root;

fn first_smoke(manifest: &mut Manifest) -> &mut SmokeCase {
    manifest
        .pairs
        .first_mut()
        .expect("manifest should contain a pair")
        .smoke
        .as_mut()
        .expect("first pair should have a smoke case")
}

#[test]
fn partial_manifest_matches_pinned_data() {
    Manifest::load()
        .expect("E2E manifest should parse")
        .validate(repository_root())
        .expect("E2E manifest should be valid");
}

#[test]
fn complete_mode_rejects_the_partial_matrix() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest.coverage = Coverage::Complete;

    let error = manifest
        .validate(repository_root())
        .expect_err("partial matrix should not satisfy complete coverage");

    assert!(error.contains("complete E2E manifest is missing languages"));
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
    let error = serde_yaml::from_str::<Manifest>(
        "schema-version: 2\ncoverage: partial\ncommands: []\nlanguages: []\npairs: []\nunknown: true\n",
    )
    .expect_err("unknown manifest fields should fail");

    assert!(error.to_string().contains("unknown field `unknown`"));
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
