use super::{install_downloaded_data, locate_data_root};
use crate::runtime_state::RuntimeState;
use crate::test_support::TestDir;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use tar::Builder;

fn archive_with_files(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut builder = Builder::new(&mut tar_bytes);
        for (path, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, *path, *contents)
                .expect("tar entry should be added");
        }
        builder.finish().expect("tar should finish");
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    use std::io::Write as _;
    encoder
        .write_all(&tar_bytes)
        .expect("gzip should accept tar bytes");
    encoder.finish().expect("gzip should finish")
}

#[test]
fn installs_valid_downloaded_data_into_runtime_data_dir() {
    let dir = TestDir::new("update-install");
    let state = RuntimeState::new(dir.path().join("state"));
    state.ensure_dirs().expect("state dirs should be created");
    let archive = archive_with_files(&[
        (
            "lsp-cli-data/filetypes/rust.yaml",
            b"extensions: [rs]\npatterns: []\n",
        ),
        (
            "lsp-cli-data/lsp/rust-analyzer.yaml",
            b"filetypes: [rust]\nroot_markers: [Cargo.toml]\nname: rust-analyzer\ncmdline: rust-analyzer\n",
        ),
        ("lsp-cli-data/lsp-cli.yaml", b"download-version: latest\n"),
    ]);

    install_downloaded_data(&state, &archive).expect("downloaded data should install");

    assert!(state.data_dir().join("filetypes/rust.yaml").is_file());
    assert!(state.data_dir().join("lsp/rust-analyzer.yaml").is_file());
}

#[test]
fn rejects_download_missing_config_directories() {
    let dir = TestDir::new("update-missing-dirs");
    let archive = archive_with_files(&[("lsp-cli-data/README.md", b"hello\n")]);
    let extracted = dir.path().join("extract");
    fs::create_dir_all(&extracted).expect("extract dir should exist");
    super::extract_archive(&extracted, &archive).expect("archive should extract");

    let error = locate_data_root(&extracted).expect_err("archive should be rejected");

    assert!(error.contains("filetypes/") && error.contains("lsp/"));
}

#[test]
fn leaves_previous_data_untouched_when_validation_fails() {
    let dir = TestDir::new("update-validation");
    let state = RuntimeState::new(dir.path().join("state"));
    state.ensure_dirs().expect("state dirs should be created");
    fs::create_dir_all(state.data_dir().join("filetypes")).expect("filetypes dir should exist");
    fs::create_dir_all(state.data_dir().join("lsp")).expect("lsp dir should exist");
    fs::write(
        state.data_dir().join("filetypes/original.yaml"),
        "extensions: [orig]\npatterns: []\n",
    )
    .expect("original filetype should exist");
    fs::write(
        state.data_dir().join("lsp/original.yaml"),
        "filetypes: [original]\nroot_markers: []\nname: original\ncmdline: original\n",
    )
    .expect("original lsp should exist");
    let archive = archive_with_files(&[
        (
            "lsp-cli-data/filetypes/bad.yaml",
            b"extensions: [bad]\npatterns: ['(']\n",
        ),
        (
            "lsp-cli-data/lsp/bad.yaml",
            b"filetypes: [bad]\nroot_markers: []\nname: bad\ncmdline: bad\n",
        ),
    ]);

    let error = install_downloaded_data(&state, &archive).expect_err("invalid data should fail");

    assert!(error.contains("invalid regex"));
    assert!(state.data_dir().join("filetypes/original.yaml").is_file());
    assert!(state.data_dir().join("lsp/original.yaml").is_file());
}
