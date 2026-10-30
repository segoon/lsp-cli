use super::DocumentLanguages;
use crate::config::{ConfigStore, load_config_store};
use crate::test_support::TestDir;
use std::collections::BTreeSet;
use std::path::Path;

fn config(files: &[(&str, &str)]) -> crate::error::Result<ConfigStore> {
    let dir = TestDir::new("document-language-config");
    dir.write_file("lsp/probe.yaml", "name: probe\ncmdline: probe\n");
    for (id, yaml) in files {
        dir.write_file(&format!("filetypes/{id}.yaml"), yaml);
    }
    load_config_store(dir.path())
}

fn languages(files: &[(&str, &str)], allowed: &[&str]) -> DocumentLanguages {
    let config = config(files).expect("valid config");
    DocumentLanguages::new(
        &config.filetypes,
        &allowed.iter().map(|id| (*id).to_string()).collect(),
    )
}

#[test]
fn resolves_default_override_case_and_filename_patterns_per_document() {
    let languages = languages(
        &[
            ("lua", "extensions: [lua]"),
            ("cs", "extensions: [cs]\nlanguage-id: csharp"),
            ("python", "extensions: [PY]"),
            (
                "bash",
                "patterns: ['^\\.bashrc$']\nlanguage-id: shellscript",
            ),
            ("make", "patterns: ['^Makefile$']\nlanguage-id: makefile"),
            ("typescriptreact", "extensions: [tsx]"),
        ],
        &["lua", "cs", "python", "bash", "make", "typescriptreact"],
    );
    for (path, expected) in [
        ("sample.lua", "lua"),
        ("sample.cs", "csharp"),
        ("sample.PY", "python"),
        (".bashrc", "shellscript"),
        ("Makefile", "makefile"),
        ("sample.tsx", "typescriptreact"),
    ] {
        assert_eq!(
            languages.resolve(Path::new(path)).expect("language"),
            expected
        );
    }
}

#[test]
fn restricts_ambiguous_extensions_to_selected_filetypes() {
    let files = [
        ("objc", "extensions: [m]\nlanguage-id: objective-c"),
        ("matlab", "extensions: [m]"),
    ];
    for (allowed, expected) in [(["objc"], "objective-c"), (["matlab"], "matlab")] {
        assert_eq!(
            languages(&files, &allowed)
                .resolve(Path::new("main.m"))
                .expect("selected language"),
            expected
        );
    }
    let error = languages(&files, &["objc", "matlab"])
        .resolve(Path::new("main.m"))
        .expect_err("ambiguous languages must not be guessed");
    assert!(error.contains("matlab, objc"));
    assert!(error.contains("--lang"));
}

#[test]
fn accepts_detection_aliases_with_the_same_protocol_id() {
    let languages = languages(
        &[
            ("yaml", "extensions: [yaml]"),
            ("yaml.custom", "extensions: [yaml]\nlanguage-id: yaml"),
        ],
        &["yaml", "yaml.custom"],
    );
    assert_eq!(
        languages.resolve(Path::new("main.yaml")).expect("alias"),
        "yaml"
    );
}

#[test]
fn rejects_unknown_or_disallowed_documents_without_plaintext_fallback() {
    let languages = languages(&[("lua", "extensions: [lua]")], &[]);
    for path in ["main.lua", "unknown", "/"] {
        let error = languages.resolve(Path::new(path)).expect_err("no language");
        assert!(error.contains("no filetype matches"));
    }
    DocumentLanguages::new(&[], &BTreeSet::new())
        .resolve(Path::new("main.py"))
        .expect_err("unconfigured languages");
}

#[test]
fn rejects_empty_language_id_overrides() {
    for id in ["''", "'   '"] {
        let error = config(&[("lua", &format!("extensions: [lua]\nlanguage-id: {id}"))])
            .expect_err("invalid override");
        assert!(error.contains("lua.yaml"));
        assert!(error.contains("language-id must not be empty"));
    }
}
