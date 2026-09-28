use std::fs;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum Digest {
    Sha256(&'static str),
    Sha512(&'static str),
}

impl Digest {
    pub(super) fn algorithm(self) -> &'static str {
        match self {
            Self::Sha256(_) => "sha256",
            Self::Sha512(_) => "sha512",
        }
    }

    pub(super) fn expected(self) -> &'static str {
        match self {
            Self::Sha256(value) | Self::Sha512(value) => value,
        }
    }
}

pub(super) struct Validation {
    pub(super) program: &'static str,
    pub(super) args: &'static [&'static str],
    pub(super) expected: &'static str,
    pub(super) include_stderr: bool,
}

pub(super) struct Dependency {
    pub(super) name: &'static str,
    pub(super) version: &'static str,
    pub(super) platform: String,
    pub(super) url: String,
    pub(super) archive_name: &'static str,
    pub(super) digest: Digest,
    pub(super) strip_components: u8,
    pub(super) path_directory: &'static str,
    pub(super) validation: Validation,
}

pub(super) fn dependencies() -> Result<Vec<Dependency>, String> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err(format!(
            "automatic E2E dependency setup supports Linux x86-64 only; this host reports {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
    let ubuntu = ubuntu_release(Path::new("/etc/os-release"))?;
    let (ruby_archive, ruby_digest) = match ubuntu.as_str() {
        "22.04" => (
            "ruby-3.3.6-ubuntu-22.04-x64.tar.gz",
            "d8bafa50f7190148473ad0d4b14b4dc3da60be7a490887092257ab2dc9b8fe5b",
        ),
        "24.04" => (
            "ruby-3.3.6-ubuntu-24.04-x64.tar.gz",
            "e7317b23328584cfa46c08c0860048e3198617a1c03f3a92783a2820738b7035",
        ),
        _ => return Err(format!("unsupported validated Ubuntu release {ubuntu:?}")),
    };
    let linux = "linux-x86_64".to_string();
    Ok(vec![
        Dependency {
            name: "go",
            version: "1.27.1",
            platform: linux.clone(),
            url: "https://go.dev/dl/go1.27.1.linux-amd64.tar.gz".to_string(),
            archive_name: "go1.27.1.linux-amd64.tar.gz",
            digest: Digest::Sha256(
                "63d339f0da5ab53635a56f2490a7984dfe12dfcff22ad749f63edaf590168445",
            ),
            strip_components: 1,
            path_directory: "bin",
            validation: Validation {
                program: "bin/go",
                args: &["version"],
                expected: "go1.27.1",
                include_stderr: false,
            },
        },
        Dependency {
            name: "java",
            version: "21.0.12.1+1",
            platform: linux.clone(),
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jdk_x64_linux_hotspot_21.0.12.1_1.tar.gz".to_string(),
            archive_name: "OpenJDK21U-jdk_x64_linux_hotspot_21.0.12.1_1.tar.gz",
            digest: Digest::Sha256(
                "ce79869e1307ed8ee1e2baa86a412b1eb5b75d10a01006d788a6f968bcfaee94",
            ),
            strip_components: 1,
            path_directory: "bin",
            validation: Validation {
                program: "bin/java",
                args: &["-version"],
                expected: "21.0.12.1",
                include_stderr: true,
            },
        },
        Dependency {
            name: "node",
            version: "24.21.0",
            platform: linux.clone(),
            url: "https://nodejs.org/dist/v24.21.0/node-v24.21.0-linux-x64.tar.xz"
                .to_string(),
            archive_name: "node-v24.21.0-linux-x64.tar.xz",
            digest: Digest::Sha256(
                "fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6",
            ),
            strip_components: 1,
            path_directory: "bin",
            validation: Validation {
                program: "bin/node",
                args: &["--version"],
                expected: "v24.21.0",
                include_stderr: false,
            },
        },
        Dependency {
            name: "dotnet",
            version: "10.0.401",
            platform: linux.clone(),
            url: "https://builds.dotnet.microsoft.com/dotnet/Sdk/10.0.401/dotnet-sdk-10.0.401-linux-x64.tar.gz".to_string(),
            archive_name: "dotnet-sdk-10.0.401-linux-x64.tar.gz",
            digest: Digest::Sha512(
                "51c8b999af9e8dd9998c9edc5944e19a90788862068acd38694e098889054ce8c23d4f0c5cccfa16bf187d044562359e5ee69a9f8ad0bbe913ba90311fbce25b",
            ),
            strip_components: 0,
            path_directory: "",
            validation: Validation {
                program: "dotnet",
                args: &["--version"],
                expected: "10.0.401",
                include_stderr: false,
            },
        },
        Dependency {
            name: "zig",
            version: "0.15.2",
            platform: linux.clone(),
            url: "https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz"
                .to_string(),
            archive_name: "zig-x86_64-linux-0.15.2.tar.xz",
            digest: Digest::Sha256(
                "02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239",
            ),
            strip_components: 1,
            path_directory: "",
            validation: Validation {
                program: "zig",
                args: &["version"],
                expected: "0.15.2",
                include_stderr: false,
            },
        },
        Dependency {
            name: "ruby",
            version: "3.3.6",
            platform: format!("ubuntu-{ubuntu}-x86_64"),
            url: format!(
                "https://github.com/ruby/ruby-builder/releases/download/ruby-3.3.6/{ruby_archive}"
            ),
            archive_name: ruby_archive,
            digest: Digest::Sha256(ruby_digest),
            strip_components: 1,
            path_directory: "bin",
            validation: Validation {
                program: "bin/ruby",
                args: &["--version"],
                expected: "ruby 3.3.6",
                include_stderr: false,
            },
        },
    ])
}

fn ubuntu_release(path: &Path) -> Result<String, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value = |name: &str| {
        contents.lines().find_map(|line| {
            line.strip_prefix(&format!("{name}="))
                .map(|value| value.trim_matches('"').to_string())
        })
    };
    let id = value("ID").unwrap_or_else(|| "unknown".to_string());
    let version = value("VERSION_ID").unwrap_or_else(|| "unknown".to_string());
    if id == "ubuntu" && matches!(version.as_str(), "22.04" | "24.04") {
        Ok(version)
    } else {
        Err(format!(
            "automatic E2E Ruby setup supports Ubuntu 22.04 and 24.04 x86-64; this host reports {id} {version}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_supported_ubuntu_releases() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let release = directory.path().join("os-release");
        fs::write(&release, "ID=ubuntu\nVERSION_ID=\"24.04\"\n")
            .expect("release fixture should be written");
        assert_eq!(
            ubuntu_release(&release).expect("Ubuntu should parse"),
            "24.04"
        );

        fs::write(&release, "ID=debian\nVERSION_ID=\"12\"\n")
            .expect("release fixture should be replaced");
        ubuntu_release(&release).expect_err("Debian should be rejected");
    }
}
