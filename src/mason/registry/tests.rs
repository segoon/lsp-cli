use super::{
    MasonAsset, MasonAssetBin, MasonNeovim, MasonPackage, MasonRegistry, MasonSource,
    MasonVersionOverride, OneOrMany,
};
use crate::runtime_state::RuntimeState;
use crate::test_support::TestDir;
use std::collections::BTreeMap;
use std::fs;

#[test]
fn keeps_only_lsp_packages_with_lspconfig_mapping() {
    let registry = MasonRegistry::from_packages(vec![
        MasonPackage {
            name: "pyright".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:npm/pyright@1.0.0".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: None,
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::from([(
                "pyright-langserver".to_string(),
                "npm:pyright-langserver".to_string(),
            )]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("pyright".to_string()),
            },
        },
        MasonPackage {
            name: "stylua".to_string(),
            categories: vec!["Formatter".to_string()],
            source: MasonSource {
                id: "pkg:github/john/stylua@1.0.0".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: None,
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::new(),
            share: BTreeMap::new(),
            neovim: MasonNeovim::default(),
        },
    ]);

    assert_eq!(
        registry
            .package_for_lspconfig("pyright")
            .expect("pyright should be indexed")
            .name,
        "pyright"
    );
    assert!(registry.package_for_lspconfig("stylua").is_none());
}

fn package(name: &str, lspconfig: Option<&str>, bin_name: &str) -> MasonPackage {
    MasonPackage {
        name: name.to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: format!("pkg:npm/{name}@1.0.0"),
            extra_packages: Vec::new(),
            asset: None,
            download: None,
            version_overrides: Vec::new(),
        },
        bin: BTreeMap::from([(bin_name.to_string(), format!("npm:{bin_name}"))]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: lspconfig.map(str::to_string),
        },
    }
}

#[test]
fn falls_back_to_package_name_and_binary_name() {
    let registry = MasonRegistry::from_packages(vec![
        package("aiken", Some("aiken_lsp"), "aiken"),
        package(
            "ada-language-server",
            Some("ada_language_server"),
            "ada_language_server",
        ),
    ]);

    assert_eq!(
        registry
            .package_for_detected("aiken", "aiken", "aiken")
            .expect("package-name fallback should resolve")
            .name,
        "aiken"
    );
    assert_eq!(
        registry
            .package_for_detected("ada_ls", "ada_language_server", "ada_language_server")
            .expect("binary-name fallback should resolve")
            .name,
        "ada-language-server"
    );
}

#[test]
fn applies_most_specific_matching_version_override() {
    let registry = MasonRegistry::from_packages(vec![MasonPackage {
        name: "angular-language-server".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:npm/@angular/language-server@17.3.2".to_string(),
            extra_packages: vec!["typescript@latest".to_string()],
            asset: None,
            download: None,
            version_overrides: vec![
                MasonVersionOverride {
                    constraint: "semver:<=19.2.4".to_string(),
                    id: "pkg:npm/@angular/language-server@19.2.4".to_string(),
                    extra_packages: Some(vec!["typescript@5.8.3".to_string()]),
                    asset: None,
                    download: None,
                },
                MasonVersionOverride {
                    constraint: "semver:<=17.3.2".to_string(),
                    id: "pkg:npm/@angular/language-server@17.3.2".to_string(),
                    extra_packages: Some(vec!["typescript@5.3.2".to_string()]),
                    asset: None,
                    download: None,
                },
            ],
        },
        bin: BTreeMap::from([("ngserver".to_string(), "npm:ngserver".to_string())]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: Some("angularls".to_string()),
        },
    }]);

    let package = registry
        .package_for_lspconfig("angularls")
        .expect("angular package should be indexed");

    assert_eq!(package.source.id, "pkg:npm/@angular/language-server@17.3.2");
    assert_eq!(package.source.extra_packages, vec!["typescript@5.3.2"]);
}

#[test]
fn applies_version_override_asset_payload() {
    let registry = MasonRegistry::from_packages(vec![MasonPackage {
        name: "rubyfmt".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:github/fables-tales/rubyfmt@v0.8.1".to_string(),
            extra_packages: Vec::new(),
            asset: Some(OneOrMany::One(MasonAsset {
                target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                file: OneOrMany::One("rubyfmt-latest.tar.gz".to_string()),
                bin: Some(MasonAssetBin::One("rubyfmt".to_string())),
                ext: None,
            })),
            download: None,
            version_overrides: vec![MasonVersionOverride {
                constraint: "semver:<=v0.8.1".to_string(),
                id: "pkg:github/fables-tales/rubyfmt@v0.8.1".to_string(),
                extra_packages: None,
                asset: Some(OneOrMany::One(MasonAsset {
                    target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                    file: OneOrMany::One("rubyfmt-v0.8.1-Linux.tar.gz".to_string()),
                    bin: Some(MasonAssetBin::One(
                        "tmp/releases/{{version}}-Linux/rubyfmt".to_string(),
                    )),
                    ext: None,
                })),
                download: None,
            }],
        },
        bin: BTreeMap::from([("rubyfmt".to_string(), "{{source.asset.bin}}".to_string())]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: Some("rubyfmt".to_string()),
        },
    }]);

    let package = registry
        .package_for_lspconfig("rubyfmt")
        .expect("rubyfmt package should be indexed");

    assert_eq!(
        package.source.assets()[0].file.as_slice()[0],
        "rubyfmt-v0.8.1-Linux.tar.gz"
    );
}

#[test]
fn parses_object_valued_asset_bin_mapping() {
    let package = serde_json::from_value::<MasonPackage>(serde_json::json!({
        "name": "kcl",
        "categories": ["LSP"],
        "source": {
            "id": "pkg:github/kcl-lang/kcl@v0.11.2",
            "asset": [{
                "target": "linux_x64_gnu",
                "file": "kclvm-v0.11.2-linux-amd64.tar.gz",
                "bin": {
                    "kcl": "exec:kclvm/bin/kclvm_cli",
                    "kcl_language_server": "exec:kclvm/bin/kcl-language-server"
                }
            }]
        },
        "bin": {
            "kcl-language-server": "{{source.asset.bin.kcl_language_server}}"
        },
        "neovim": {
            "lspconfig": "kcl"
        }
    }))
    .expect("package should parse");

    let asset_bin = package.source.assets()[0]
        .bin
        .as_ref()
        .and_then(MasonAssetBin::as_map)
        .expect("object-valued bin should parse as map");

    assert_eq!(
        asset_bin.get("kcl_language_server"),
        Some(&"exec:kclvm/bin/kcl-language-server".to_string())
    );
}

#[test]
fn load_cached_returns_none_when_registry_is_missing() {
    let dir = TestDir::new("mason-registry");
    let state = RuntimeState::new(dir.path().join("state"));

    assert!(MasonRegistry::load_cached(&state).is_none());
}

#[test]
fn load_cached_returns_none_for_corrupted_registry() {
    let dir = TestDir::new("mason-registry");
    let state = RuntimeState::new(dir.path().join("state"));
    fs::create_dir_all(state.registry_dir()).expect("registry dir should be created");
    fs::write(state.registry_json_path(), b"{not json]")
        .expect("corrupted registry should be written");

    assert!(MasonRegistry::load_cached(&state).is_none());
}

#[test]
fn registry_metadata_freshness_respects_threshold() {
    let metadata = super::cache::RegistryMetadata {
        release_tag: "2026-01-01".to_string(),
        refreshed_at_epoch_seconds: 10,
        digest: None,
    };

    assert!(metadata.is_fresh_at(10 + 30 * 24 * 60 * 60));
    assert!(!metadata.is_fresh_at(11 + 30 * 24 * 60 * 60));
}
