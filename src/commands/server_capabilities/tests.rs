use super::render_for_tests;
use crate::lsp::InitializeResponse;
use serde_json::json;

fn initialize_response(value: serde_json::Value) -> InitializeResponse {
    InitializeResponse::from_raw_value(value).expect("initialize response should decode")
}

fn lines(items: &[&str]) -> String {
    items.join("\n")
}

#[test]
fn renders_yaml_like_capabilities_output() {
    let initialize = initialize_response(json!({
        "capabilities": {
            "positionEncoding": "utf-8",
            "textDocumentSync": {
                "openClose": true,
                "change": 2,
                "save": {
                    "includeText": false
                }
            },
            "declarationProvider": true,
            "definitionProvider": false,
            "referencesProvider": true,
            "documentHighlightProvider": true,
            "hoverProvider": true,
            "colorProvider": {
                "workDoneProgress": true
            },
            "workspace": {
                "workspaceFolders": {
                    "supported": true,
                    "changeNotifications": "workspace-folders"
                }
            },
            "experimental": {
                "mylsp": {
                    "dostuff": true
                }
            }
        },
        "serverInfo": {
            "name": "clangd",
            "version": "1.2.3"
        }
    }));

    assert_eq!(
        render_for_tests(
            &[
                "/usr/bin/clangd".to_string(),
                "--background-index".to_string(),
                "-j4".to_string(),
            ],
            "clangd",
            &initialize,
        ),
        lines(&[
            "cmdline: /usr/bin/clangd --background-index -j4",
            "server: clangd (1.2.3)",
            "capabilities:",
            "  position encoding: utf-8",
            "  text document sync: yes",
            "    open/close: yes",
            "    change: incremental",
            "    save: yes",
            "      include text on save: no",
            "  notebook document sync: no",
            "  selection ranges: no",
            "  hover: yes",
            "  completion: no",
            "  signature help: no",
            "  goto definition: no",
            "  goto type definition: no",
            "  goto implementation: no",
            "  find references: yes",
            "  document highlights: yes",
            "  document symbols: no",
            "  workspace symbols: no",
            "  code actions: no",
            "  code lens: no",
            "  document formatting: no",
            "  document range formatting: no",
            "  document on-type formatting: no",
            "  rename: no",
            "  document links: no",
            "  document color: yes",
            "    work done progress: yes",
            "  folding ranges: no",
            "  goto declaration: yes",
            "  execute command: no",
            "  workspace folders: yes",
            "    change notifications: workspace-folders",
            "    supported: yes",
            "  workspace file create notifications: no",
            "  workspace file create preparation: no",
            "  workspace file rename notifications: no",
            "  workspace file rename preparation: no",
            "  workspace file delete notifications: no",
            "  workspace file delete preparation: no",
            "  call hierarchy: no",
            "  semantic tokens: no",
            "  monikers: no",
            "  linked editing ranges: no",
            "  inline values: no",
            "  inlay hints: no",
            "  pull diagnostics: no",
            "  experimental/mylsp/dostuff: yes",
        ])
    );
}

#[test]
fn defaults_position_encoding_and_omits_missing_server_version() {
    let initialize = initialize_response(json!({
        "capabilities": {
            "textDocumentSync": 1
        },
        "serverInfo": {
            "name": "test-lsp"
        }
    }));

    assert!(
        render_for_tests(&["test-lsp".to_string()], "fallback", &initialize).contains(&lines(&[
            "server: test-lsp",
            "capabilities:",
            "  position encoding: utf-16",
            "  text document sync: yes",
            "    change: full",
        ]))
    );
}

#[test]
fn renders_unknown_raw_capabilities() {
    let initialize = initialize_response(json!({
        "capabilities": {
            "customProvider": {
                "enabled": true,
                "mode": "fast"
            }
        }
    }));

    assert!(
        render_for_tests(&["custom-lsp".to_string()], "custom-lsp", &initialize).contains(&lines(
            &[
                "  customProvider: yes",
                "    enabled: yes",
                "    mode: fast",
            ]
        ))
    );
}
