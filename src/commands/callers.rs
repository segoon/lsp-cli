use crate::cli::SymbolQueryArgs;
use crate::commands::symbol_query::{render_workspace_symbol_result, run_callers_query};
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(args: &SymbolQueryArgs, config: &ConfigStore) -> Result<String> {
    let result = run_callers_query(&args.query, &args.name, config)?;
    Ok(render_workspace_symbol_result(
        &args.name,
        &args.query.query,
        args.query.files_with_matches,
        false,
        result,
    ))
}
