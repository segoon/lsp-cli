pub struct TemplateContext<'a> {
    pub version: &'a str,
    pub source_asset_bin: Option<&'a str>,
    pub source_asset_file: Option<&'a str>,
    pub source_download_bin: Option<&'a str>,
    pub source_download_config: Option<&'a str>,
}

impl TemplateContext<'_> {
    #[must_use]
    pub fn render(&self, input: &str) -> String {
        let mut rendered = input.replace("{{version}}", self.version);
        rendered = rendered.replace(
            "{{ version | strip_prefix \"v\" }}",
            self.version.strip_prefix('v').unwrap_or(self.version),
        );

        if let Some(value) = self.source_asset_bin {
            rendered = rendered.replace("{{source.asset.bin}}", value);
        }
        if let Some(value) = self.source_asset_file {
            rendered = rendered.replace("{{source.asset.file}}", value);
        }
        if let Some(value) = self.source_download_bin {
            rendered = rendered.replace("{{source.download.bin}}", value);
        }
        if let Some(value) = self.source_download_config {
            rendered = rendered.replace("{{source.download.config}}", value);
        }

        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::TemplateContext;

    #[test]
    fn renders_common_mason_templates() {
        let context = TemplateContext {
            version: "v1.2.3",
            source_asset_bin: Some("exec:libexec/bin/server"),
            source_asset_file: Some("server-v1.2.3.tar.gz"),
            source_download_bin: Some("bzl"),
            source_download_config: Some("config_linux/"),
        };

        assert_eq!(context.render("{{version}}"), "v1.2.3");
        assert_eq!(context.render("{{ version | strip_prefix \"v\" }}"), "1.2.3");
        assert_eq!(context.render("{{source.asset.bin}}"), "exec:libexec/bin/server");
        assert_eq!(context.render("{{source.asset.file}}"), "server-v1.2.3.tar.gz");
        assert_eq!(context.render("{{source.download.bin}}"), "bzl");
        assert_eq!(context.render("{{source.download.config}}"), "config_linux/");
    }
}
