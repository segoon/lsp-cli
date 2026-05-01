use crate::cli::SymbolQueryArgs;
use crate::commands::symbol_query::{
    render_symbol_matches_text, render_workspace_symbol_json, run_declaration_query, truncate_items,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &SymbolQueryArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_declaration_query(&args.query, &args.name, config)?;
    let matches = truncate_items(
        result.matches,
        args.query.limit,
        if args.query.json { "items" } else { "lines" },
    );

    Ok(if args.query.json {
        render_workspace_symbol_json(
            &args.name,
            &args.query.directory,
            &result.detected_filetypes,
            &result.server,
            &matches,
        )
    } else {
        render_symbol_matches_text(&matches)
    })
}
