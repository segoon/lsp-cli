use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub struct TemplateContext<'a> {
    pub version: &'a str,
    pub source_asset_bin: Option<&'a str>,
    pub source_asset_file: Option<&'a str>,
    pub source_asset_ext: Option<&'a str>,
    pub source_download_bin: Option<&'a str>,
    pub source_download_config: Option<&'a str>,
    pub source_download_man: Option<&'a str>,
    pub source_asset_named_bins: BTreeMap<String, String>,
}

impl TemplateContext<'_> {
    #[must_use]
    pub fn render(&self, input: &str) -> String {
        template_regex()
            .replace_all(input, |captures: &regex::Captures<'_>| {
                self.render_expression(captures.get(1).map_or("", |capture| capture.as_str()))
                    .unwrap_or_else(|| captures[0].to_string())
            })
            .into_owned()
    }

    fn render_expression(&self, expression: &str) -> Option<String> {
        let expression = expression.trim();

        match expression {
            "version" => Some(self.version.to_string()),
            "version | strip_prefix \"v\"" => {
                Some(self.version.strip_prefix('v').unwrap_or(self.version).to_string())
            }
            "source.asset.bin" => Some(self.source_asset_bin.unwrap_or("").to_string()),
            "source.asset.file" => Some(self.source_asset_file.unwrap_or("").to_string()),
            "source.asset.ext" => Some(self.source_asset_ext.unwrap_or("").to_string()),
            "source.download.bin" => Some(self.source_download_bin.unwrap_or("").to_string()),
            "source.download.config" => Some(self.source_download_config.unwrap_or("").to_string()),
            "source.download.man" => Some(self.source_download_man.unwrap_or("").to_string()),
            _ => expression
                .strip_prefix("source.asset.bin.")
                .and_then(|name| self.source_asset_named_bins.get(name).cloned()),
        }
    }

    #[must_use]
    pub(crate) fn empty() -> Self {
        Self {
            version: "",
            source_asset_bin: None,
            source_asset_file: None,
            source_asset_ext: None,
            source_download_bin: None,
            source_download_config: None,
            source_download_man: None,
            source_asset_named_bins: BTreeMap::new(),
        }
    }
}

fn template_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\{\{\s*(.*?)\s*\}\}").expect("template regex should compile"))
}

#[cfg(test)]
mod tests {
    use super::TemplateContext;
    use std::collections::BTreeMap;

    #[test]
    fn renders_common_mason_templates() {
        let context = TemplateContext {
            version: "v1.2.3",
            source_asset_bin: Some("exec:libexec/bin/server"),
            source_asset_file: Some("server-v1.2.3.tar.gz"),
            source_asset_ext: Some(".exe"),
            source_download_bin: Some("bzl"),
            source_download_config: Some("config_linux/"),
            source_download_man: Some("quick-lint-js/share/man/"),
            source_asset_named_bins: BTreeMap::from([(
                "lsp".to_string(),
                "exec:language_server.sh".to_string(),
            )]),
        };

        assert_eq!(context.render("{{version}}"), "v1.2.3");
        assert_eq!(context.render("{{ version | strip_prefix \"v\" }}"), "1.2.3");
        assert_eq!(context.render("{{source.asset.bin}}"), "exec:libexec/bin/server");
        assert_eq!(context.render("{{source.asset.file}}"), "server-v1.2.3.tar.gz");
        assert_eq!(context.render("tool{{source.asset.ext}}"), "tool.exe");
        assert_eq!(context.render("{{source.download.bin}}"), "bzl");
        assert_eq!(context.render("{{source.download.config}}"), "config_linux/");
        assert_eq!(
            context.render("{{source.download.man}}"),
            "quick-lint-js/share/man/"
        );
        assert_eq!(
            context.render("{{ source.asset.bin.lsp }}"),
            "exec:language_server.sh"
        );
    }

    #[test]
    fn renders_missing_known_values_as_empty_strings() {
        let context = TemplateContext::empty();

        assert_eq!(context.render("ast-grep{{source.asset.ext}}"), "ast-grep");
        assert_eq!(context.render("{{source.download.man}}"), "");
    }
}
