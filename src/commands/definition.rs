use crate::cli::DefinitionArgs;
use crate::commands::symbol_query::{render_workspace_symbol_result, run_definition_query};
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(args: &DefinitionArgs, config: &ConfigStore) -> Result<String> {
    let result = run_definition_query(&args.query, &args.name, args.full, config)?;
    Ok(render_workspace_symbol_result(
        &args.name,
        &args.query.query,
        args.query.files_with_matches,
        args.full,
        result,
    ))
}
