#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CargoSource {
    pub package_name: String,
    pub version: String,
    pub repository_url: Option<String>,
    pub rev: bool,
    pub features: Option<String>,
    pub locked: bool,
}

impl CargoSource {
    pub(super) fn new(package_name: String, version: String, qualifiers: Option<&str>) -> Self {
        let mut source = Self {
            package_name,
            version,
            repository_url: None,
            rev: false,
            features: None,
            locked: true,
        };
        for (key, value) in qualifiers
            .into_iter()
            .flat_map(|raw| url::form_urlencoded::parse(raw.as_bytes()))
        {
            match key.as_ref() {
                "repository_url" => source.repository_url = Some(value.into_owned()),
                "rev" => source.rev = value == "true",
                "features" => source.features = Some(value.into_owned()),
                "locked" => source.locked = value != "false",
                _ => {}
            }
        }
        source
    }
}

#[cfg(test)]
mod tests {
    use crate::mason::source::{SourceId, parse_source_id};

    #[test]
    fn preserves_cargo_qualifiers_and_mason_defaults() {
        for (qualifiers, repository, rev, features, locked) in [
            ("", None, false, None, true),
            (
                "?repository_url=https%3A%2F%2Fexample.com%2Frepo%3Fx%3D1%26y%3D2",
                Some("https://example.com/repo?x=1&y=2"),
                false,
                None,
                true,
            ),
            (
                "?repository_url=https://example.com/repo&rev=true&locked=false&features=lsp%2Ccli",
                Some("https://example.com/repo"),
                true,
                Some("lsp,cli"),
                false,
            ),
            (
                "?rev=false&locked=true&features=lsp",
                None,
                false,
                Some("lsp"),
                true,
            ),
        ] {
            let id = format!("pkg:cargo/server%2Dname@v1.0.0{qualifiers}");
            let SourceId::Cargo(source) = parse_source_id(&id).expect("Cargo source should parse")
            else {
                panic!("expected Cargo source")
            };
            assert_eq!(source.package_name, "server-name");
            assert_eq!(source.version, "v1.0.0");
            assert_eq!(source.repository_url.as_deref(), repository);
            assert_eq!(source.rev, rev);
            assert_eq!(source.features.as_deref(), features);
            assert_eq!(source.locked, locked);
        }
    }
}
