use crate::cli::ServersArgs;
use crate::config::ConfigStore;
use crate::error::{Error, Result};
use std::collections::BTreeSet;

pub(super) fn run(args: &ServersArgs, config: &ConfigStore) -> Result<String> {
    if let Some(language) = args.lang.as_deref()
        && !config
            .filetypes
            .iter()
            .any(|filetype| filetype.id == language)
    {
        return Err(Error::invalid_input(format!("unsupported language {language:?}")));
    }

    let servers = config
        .lsps
        .iter()
        .filter(|lsp| {
            args.lang
                .as_deref()
                .is_none_or(|language| lsp.filetypes.iter().any(|filetype| filetype == language))
        })
        .map(|lsp| lsp.name.as_str())
        .collect::<BTreeSet<_>>();
    Ok(servers.into_iter().collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::cli::ServersArgs;
    use crate::config::{CliConfig, ConfigStore, FiletypeConfig, LspConfig};
    use crate::error::Error;

    fn config() -> ConfigStore {
        ConfigStore {
            filetypes: vec![
                FiletypeConfig {
                    id: "python".to_string(),
                    extensions: vec!["py".to_string()],
                    patterns: Vec::new(),
                },
                FiletypeConfig {
                    id: "rust".to_string(),
                    extensions: vec!["rs".to_string()],
                    patterns: Vec::new(),
                },
            ],
            lsps: vec![
                LspConfig {
                    id: "pyright".to_string(),
                    filetypes: vec!["python".to_string()],
                    root_markers: Vec::new(),
                    name: "pyright".to_string(),
                    cmdline: "pyright-langserver --stdio".to_string(),
                    wait_for_index: false,
                },
                LspConfig {
                    id: "ruff".to_string(),
                    filetypes: vec!["python".to_string()],
                    root_markers: Vec::new(),
                    name: "ruff".to_string(),
                    cmdline: "ruff server".to_string(),
                    wait_for_index: false,
                },
                LspConfig {
                    id: "rust_analyzer".to_string(),
                    filetypes: vec!["rust".to_string()],
                    root_markers: Vec::new(),
                    name: "rust-analyzer".to_string(),
                    cmdline: "rust-analyzer".to_string(),
                    wait_for_index: false,
                },
                LspConfig {
                    id: "rust_analyzer_alt".to_string(),
                    filetypes: vec!["rust".to_string()],
                    root_markers: Vec::new(),
                    name: "rust-analyzer".to_string(),
                    cmdline: "rust-analyzer --stdio".to_string(),
                    wait_for_index: false,
                },
            ],
            cli: CliConfig::default(),
        }
    }

    #[test]
    fn renders_sorted_deduplicated_servers() {
        assert_eq!(
            run(&ServersArgs { lang: None }, &config()).expect("servers should render"),
            "pyright\nruff\nrust-analyzer"
        );
    }

    #[test]
    fn filters_servers_by_language() {
        assert_eq!(
            run(
                &ServersArgs {
                    lang: Some("python".to_string())
                },
                &config(),
            )
            .expect("filtered servers should render"),
            "pyright\nruff"
        );
    }

    #[test]
    fn errors_for_unsupported_language() {
        assert!(matches!(
            run(
                &ServersArgs {
                    lang: Some("unknown".to_string())
                },
                &config(),
            ),
            Err(Error::InvalidInput(message)) if message == "unsupported language \"unknown\""
        ));
    }
}
