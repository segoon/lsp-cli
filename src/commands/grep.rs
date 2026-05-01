use crate::cli::GrepArgs;
use crate::commands::symbol_query::{
    render_symbol_matches_text, render_workspace_symbol_json, run_workspace_symbol_query,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &GrepArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_workspace_symbol_query(&args.query, &args.pattern, config)?;

    Ok(if args.query.json {
        render_workspace_symbol_json(
            &args.pattern,
            &args.query.directory,
            &result.detected_filetypes,
            &result.server,
            &result.matches,
        )
    } else {
        render_symbol_matches_text(&result.matches)
    })
}
