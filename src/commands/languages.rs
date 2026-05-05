use crate::cli::LanguagesArgs;
use crate::config::ConfigStore;
use crate::error::Result;
use std::collections::BTreeSet;

#[allow(clippy::unnecessary_wraps)]
pub(super) fn run(_args: &LanguagesArgs, config: &ConfigStore) -> Result<String> {
    let languages = config
        .filetypes
        .iter()
        .map(|filetype| filetype.id.as_str())
        .collect::<BTreeSet<_>>();
    Ok(languages.into_iter().collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::cli::LanguagesArgs;
    use crate::config::{CliConfig, ConfigStore, FiletypeConfig};
    use regex::Regex;

    #[test]
    fn renders_sorted_languages() {
        let config = ConfigStore {
            filetypes: vec![
                FiletypeConfig {
                    id: "rust".to_string(),
                    extensions: vec!["rs".to_string()],
                    patterns: Vec::new(),
                },
                FiletypeConfig {
                    id: "c".to_string(),
                    extensions: vec!["c".to_string()],
                    patterns: vec![Regex::new("main").expect("regex should compile")],
                },
            ],
            lsps: Vec::new(),
            cli: CliConfig::default(),
        };

        assert_eq!(
            run(&LanguagesArgs, &config).expect("languages should render"),
            "c\nrust"
        );
    }
}
