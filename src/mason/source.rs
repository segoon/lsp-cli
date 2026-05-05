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

pub(crate) fn parse_source_id(source_id: &str) -> Result<SourceId, String> {
    let without_prefix = source_id
        .strip_prefix("pkg:")
        .ok_or_else(|| format!("unsupported Mason package source {source_id}: missing `pkg:` prefix"))?;
    let (package_ref, version_with_qualifiers) = without_prefix
        .rsplit_once('@')
        .ok_or_else(|| format!("unsupported Mason package source {source_id}: missing `@version`"))?;
    let (kind, name) = package_ref
        .split_once('/')
        .ok_or_else(|| format!("unsupported Mason package source {source_id}: missing source kind or package name"))?;
    let decoded_name = percent_decode_component(name)
        .ok_or_else(|| format!("unsupported Mason package source {source_id}: invalid percent-encoding in package name"))?;

    let (version, qualifiers) = split_version_qualifiers(version_with_qualifiers);
    let version = version.to_string();
    let decoded_name = decoded_name.clone();

    Ok(match kind {
        "npm" => {
            SourceId::Npm {
                package_name: decoded_name,
                version,
            }
        }
        "pypi" => {
            SourceId::Pypi {
                package_name: decoded_name,
                version,
                extras: parse_pypi_extras(qualifiers),
            }
        }
        "cargo" => {
            SourceId::Cargo {
                package_name: decoded_name,
                version,
            }
        }
        "golang" => SourceId::Golang {
            module_path: decoded_name,
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

// Q: inline the function
fn split_version_qualifiers(version_with_qualifiers: &str) -> (&str, Option<&str>) {
    match version_with_qualifiers.split_once('?') {
        Some((version, qualifiers)) => (version, Some(qualifiers)),
        None => (version_with_qualifiers, None),
    }
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
    let mut decoded = String::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = decode_hex_digit(bytes[index + 1])?;
            let low = decode_hex_digit(bytes[index + 2])?;
            decoded.push((high << 4 | low) as char);
            index += 3;
            continue;
        }

        decoded.push(bytes[index] as char);
        index += 1;
    }

    Some(decoded)
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
}
