use lsp_types::{
    SemanticTokenType, SemanticTokensClientCapabilities, SemanticTokensClientCapabilitiesRequests,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensServerCapabilities,
    TokenFormat,
};

pub(crate) fn client_capabilities() -> SemanticTokensClientCapabilities {
    SemanticTokensClientCapabilities {
        requests: SemanticTokensClientCapabilitiesRequests {
            full: Some(SemanticTokensFullOptions::Bool(true)),
            range: Some(true),
        },
        token_types: [
            "namespace",
            "type",
            "class",
            "enum",
            "interface",
            "struct",
            "typeParameter",
            "parameter",
            "variable",
            "property",
            "enumMember",
            "event",
            "function",
            "method",
            "macro",
            "keyword",
            "modifier",
            "comment",
            "string",
            "number",
            "regexp",
            "operator",
            "decorator",
        ]
        .into_iter()
        .map(SemanticTokenType::new)
        .collect(),
        token_modifiers: Vec::new(),
        formats: vec![TokenFormat::RELATIVE],
        dynamic_registration: Some(false),
        overlapping_token_support: Some(false),
        multiline_token_support: Some(false),
        augments_syntax_tokens: Some(false),
        ..Default::default()
    }
}

pub(crate) fn options(provider: &SemanticTokensServerCapabilities) -> &SemanticTokensOptions {
    match provider {
        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => options,
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
            &options.semantic_tokens_options
        }
    }
}

pub(crate) fn supports_full(options: &SemanticTokensOptions) -> bool {
    matches!(
        options.full,
        Some(SemanticTokensFullOptions::Bool(true) | SemanticTokensFullOptions::Delta { .. })
    )
}
