use crate::cli::ListSymbolsArgs;
use crate::commands::symbol_query::{
    render_symbol_names_text, render_workspace_symbol_json, run_document_symbol_query,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &ListSymbolsArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_document_symbol_query(&args.query, config)?;

    Ok(if args.query.json {
        render_workspace_symbol_json(
            "",
            &args.query.directory,
            &result.detected_filetypes,
            &result.server,
            &result.matches,
        )
    } else {
        render_symbol_names_text(&result.matches)
    })
}
