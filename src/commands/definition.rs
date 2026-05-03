use crate::cli::DefinitionArgs;
use crate::commands::symbol_query::{
    render_symbol_matches_text, render_symbol_matches_text_full, render_workspace_symbol_json,
    render_workspace_symbol_json_full, run_definition_query, truncate_items,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &DefinitionArgs, config: &ConfigStore) -> Result<String, String> {
    let query = &args.query.query;
    let result = run_definition_query(&args.query, &args.name, args.full, config)?;
    let matches = truncate_items(
        result.matches,
        query.limit,
        if query.json { "items" } else { "lines" },
    );

    Ok(if query.json {
        if args.full {
            render_workspace_symbol_json_full(
                &args.name,
                &query.directory,
                &result.detected_filetypes,
                &result.server,
                &matches,
            )
        } else {
            render_workspace_symbol_json(
                &args.name,
                &query.directory,
                &result.detected_filetypes,
                &result.server,
                &matches,
            )
        }
    } else if args.full {
        render_symbol_matches_text_full(&matches)
    } else {
        render_symbol_matches_text(&matches)
    })
}
