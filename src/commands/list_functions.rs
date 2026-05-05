use crate::cli::ListFunctionsArgs;
use crate::commands::symbol_query::{
    render_symbol_names_text, render_workspace_symbol_json, run_document_symbol_query,
    truncate_items,
};
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(args: &ListFunctionsArgs, config: &ConfigStore) -> Result<String> {
    let query = &args.query.query;
    let result = run_document_symbol_query(&args.query, config)?;
    let matches = truncate_items(
        result.matches,
        query.limit,
        if query.json { "items" } else { "lines" },
    );

    Ok(if query.json {
        render_workspace_symbol_json(
            "",
            &query.directory,
            &result.detected_filetypes,
            &result.server,
            &matches,
        )
    } else {
        render_symbol_names_text(&matches)
    })
}
