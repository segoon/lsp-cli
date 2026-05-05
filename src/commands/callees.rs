use crate::cli::SymbolQueryArgs;
use crate::commands::symbol_query::{
    render_symbol_match_paths_text, render_symbol_matches_text, render_workspace_symbol_json,
    run_callees_query, truncate_items,
};
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(args: &SymbolQueryArgs, config: &ConfigStore) -> Result<String> {
    let query = &args.query.query;
    let result = run_callees_query(&args.query, &args.name, config)?;
    let matches = truncate_items(
        result.matches,
        query.limit,
        if query.json { "items" } else { "lines" },
    );

    Ok(if query.json {
        render_workspace_symbol_json(
            &args.name,
            &query.directory,
            &result.detected_filetypes,
            &result.server,
            &matches,
        )
    } else if args.query.files_with_matches {
        render_symbol_match_paths_text(&matches)
    } else {
        render_symbol_matches_text(&matches)
    })
}
