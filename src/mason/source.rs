use crate::error::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceId {
    Npm {
        package_name: String,
        version: String,
    },
    Pypi {
        package_name: String,
        version: String,
        extras: Vec<String>,
    },
    Cargo {
        package_name: String,
        version: String,
    },
    Golang {
        module_path: String,
        version: String,
        subpath: Option<String>,
    },
    Nuget {
        package_name: String,
        version: String,
    },
    Github {
        repository: String,
        version: String,
    },
    Generic {
        package_name: String,
        version: String,
    },
    Unsupported {
        kind: String,
    },
}

pub(crate) fn parse_source_id(source_id: &str) -> Result<SourceId> {
    let Some(without_prefix) = source_id.strip_prefix("pkg:") else {
        return Err(Error::unexpected(format!(
            "unsupported Mason package source {source_id}: missing `pkg:` prefix"
        )));
    };
    let (without_subpath, subpath) = match without_prefix.split_once('#') {
        Some((source, subpath)) => (source, Some(parse_subpath(source_id, subpath)?)),
        None => (without_prefix, None),
    };
    let (package_and_version, qualifiers) = match without_subpath.split_once('?') {
        Some((source, qualifiers)) => (source, Some(qualifiers)),
        None => (without_subpath, None),
    };
    let Some((package_ref, encoded_version)) = package_and_version.rsplit_once('@') else {
        return Err(Error::unexpected(format!(
            "unsupported Mason package source {source_id}: missing `@version`"
        )));
    };
    let Some((kind, name)) = package_ref.split_once('/') else {
        return Err(Error::unexpected(format!(
            "unsupported Mason package source {source_id}: missing source kind or package name"
        )));
    };
    let Some(decoded_name) = percent_decode_component(name) else {
        return Err(Error::unexpected(format!(
            "unsupported Mason package source {source_id}: invalid percent-encoding in package name"
        )));
    };
    let Some(version) = percent_decode_component(encoded_version) else {
        return Err(Error::unexpected(format!(
            "unsupported Mason package source {source_id}: invalid percent-encoding in version"
        )));
    };

    if subpath.is_some() && kind != "golang" {
        return Err(Error::unexpected(format!(
            "unsupported Mason package source {source_id}: backend {kind:?} does not support a source subpath"
        )));
    }
    Ok(match kind {
        "npm" => SourceId::Npm {
            package_name: decoded_name,
            version,
        },
        "pypi" => SourceId::Pypi {
            package_name: decoded_name,
            version,
            extras: parse_pypi_extras(qualifiers),
        },
        "cargo" => SourceId::Cargo {
            package_name: decoded_name,
            version,
        },
        "golang" => SourceId::Golang {
            module_path: decoded_name,
            version,
            subpath,
        },
        "nuget" => SourceId::Nuget {
            package_name: decoded_name,
            version,
        },
        "github" => SourceId::Github {
            repository: decoded_name,
            version,
        },
        "generic" => SourceId::Generic {
            package_name: decoded_name,
            version,
        },
        _ => SourceId::Unsupported {
            kind: kind.to_string(),
        },
    })
}

fn parse_subpath(source_id: &str, raw: &str) -> Result<String> {
    if raw.is_empty() || raw.starts_with(['/', '\\']) {
        return Err(invalid_subpath(source_id));
    }
    raw.split('/')
        .map(|segment| {
            let Some(segment) = percent_decode_component(segment) else {
                return Err(Error::unexpected(format!(
                    "unsupported Mason package source {source_id}: invalid percent-encoding in subpath"
                )));
            };
            if segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains(['/', '\\'])
            {
                Err(invalid_subpath(source_id))
            } else {
                Ok(segment)
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(|segments| segments.join("/"))
}

fn invalid_subpath(source_id: &str) -> Error {
    Error::unexpected(format!(
        "unsupported Mason package source {source_id}: subpath must be relative and contain no empty, `.`, `..`, or encoded separator segments"
    ))
}

fn parse_pypi_extras(qualifiers: Option<&str>) -> Vec<String> {
    qualifiers
        .into_iter()
        .flat_map(|qualifiers| url::form_urlencoded::parse(qualifiers.as_bytes()))
        .filter_map(|(key, value)| (key == "extra").then(|| value.into_owned()))
        .collect()
}

fn percent_decode_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = decode_hex_digit(bytes[index + 1])?;
            let low = decode_hex_digit(bytes[index + 2])?;
            decoded.push(high << 4 | low);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(decoded).ok()
}

fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceId, parse_source_id};

    #[test]
    fn parses_supported_source_ids() {
        assert_eq!(
            parse_source_id("pkg:npm/pyright@1.1.409").expect("npm source should parse"),
            SourceId::Npm {
                package_name: "pyright".to_string(),
                version: "1.1.409".to_string(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:npm/%40angular/language-server@21.2.11")
                .expect("scoped npm source should parse"),
            SourceId::Npm {
                package_name: "@angular/language-server".to_string(),
                version: "21.2.11".to_string(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:pypi/jedi-language-server@0.46.0")
                .expect("pypi source should parse"),
            SourceId::Pypi {
                package_name: "jedi-language-server".to_string(),
                version: "0.46.0".to_string(),
                extras: Vec::new(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:pypi/python-lsp-server@1.14.0?extra=all")
                .expect("pypi source with extras should parse"),
            SourceId::Pypi {
                package_name: "python-lsp-server".to_string(),
                version: "1.14.0".to_string(),
                extras: vec!["all".to_string()],
            }
        );
        assert_eq!(
            parse_source_id("pkg:cargo/asm-lsp@0.10.1").expect("cargo source should parse"),
            SourceId::Cargo {
                package_name: "asm-lsp".to_string(),
                version: "0.10.1".to_string(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:golang/golang.org/x/tools/gopls@v0.21.1")
                .expect("golang source should parse"),
            SourceId::Golang {
                module_path: "golang.org/x/tools/gopls".to_string(),
                version: "v0.21.1".to_string(),
                subpath: None,
            }
        );
        assert_eq!(
            parse_source_id("pkg:nuget/roslyn-language-server@5.11.0-1.26380.4")
                .expect("NuGet source should parse"),
            SourceId::Nuget {
                package_name: "roslyn-language-server".to_string(),
                version: "5.11.0-1.26380.4".to_string(),
            }
        );
    }

    #[test]
    fn preserves_unsupported_source_kind() {
        assert_eq!(
            parse_source_id("pkg:gem/solargraph@0.50.0").expect("gem source should parse"),
            SourceId::Unsupported {
                kind: "gem".to_string(),
            }
        );
    }

    #[test]
    fn parses_and_decodes_golang_subpath_after_qualifiers() {
        assert_eq!(
            parse_source_id(
                "pkg:golang/cuelang.org/%67o@v0.17.1%2Bmeta?repository_url=ignored#cmd/%63ue"
            )
            .expect("Go source with subpath should parse"),
            SourceId::Golang {
                module_path: "cuelang.org/go".to_string(),
                version: "v0.17.1+meta".to_string(),
                subpath: Some("cmd/cue".to_string()),
            }
        );
        assert_eq!(
            parse_source_id("pkg:golang/github.com/dagger/cuelsp@v0.3.4#cmd/cuelsp")
                .expect("nested Go command source should parse"),
            SourceId::Golang {
                module_path: "github.com/dagger/cuelsp".to_string(),
                version: "v0.3.4".to_string(),
                subpath: Some("cmd/cuelsp".to_string()),
            }
        );
    }

    #[test]
    fn percent_decoder_preserves_utf8_components() {
        assert_eq!(
            parse_source_id("pkg:golang/example.com/%E2%98%83@v1.0.0#cmd/%E2%98%83")
                .expect("UTF-8 source should parse"),
            SourceId::Golang {
                module_path: "example.com/☃".to_string(),
                version: "v1.0.0".to_string(),
                subpath: Some("cmd/☃".to_string()),
            }
        );
    }

    #[test]
    fn rejects_invalid_or_unsupported_source_subpaths() {
        for source_id in [
            "pkg:golang/example.com/tool@v1.0.0#",
            "pkg:golang/example.com/tool@v1.0.0#/cmd/tool",
            "pkg:golang/example.com/tool@v1.0.0#cmd//tool",
            "pkg:golang/example.com/tool@v1.0.0#cmd/./tool",
            "pkg:golang/example.com/tool@v1.0.0#cmd/../tool",
            "pkg:golang/example.com/tool@v1.0.0#cmd/%2e%2e/tool",
            "pkg:golang/example.com/tool@v1.0.0#cmd%2ftool",
            r"pkg:golang/example.com/tool@v1.0.0#cmd\tool",
        ] {
            let error = parse_source_id(source_id)
                .expect_err("unsafe or ambiguous Go subpath should be rejected")
                .to_string();
            assert!(error.contains(source_id));
            assert!(error.contains("subpath"));
        }

        let error = parse_source_id("pkg:github/example/tool@v1.0.0#bin/tool")
            .expect_err("non-Go source subpath should be rejected")
            .to_string();
        assert!(error.contains("backend \"github\" does not support a source subpath"));
    }

    #[test]
    fn rejects_invalid_percent_encoding_in_version_or_subpath() {
        for source_id in [
            "pkg:golang/example.com/tool@v1.0.%ZZ",
            "pkg:golang/example.com/tool@v1.0.0#cmd/%ZZ",
        ] {
            let error = parse_source_id(source_id)
                .expect_err("invalid percent encoding should be rejected")
                .to_string();
            assert!(error.contains(source_id));
            assert!(error.contains("invalid percent-encoding"));
        }
    }
}
