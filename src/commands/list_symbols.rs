use crate::cli::ListSymbolsArgs;
use crate::commands::symbol_query::{
    ListSymbolsTarget, list_symbols_target, render_list_symbols_json, render_symbol_names_text,
    run_list_symbols_query, truncate_items,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &ListSymbolsArgs, config: &ConfigStore) -> Result<String, String> {
    let target = list_symbols_target(&args.path)?;
    let result = run_list_symbols_query(args, config)?;
    let matches = truncate_items(
        result.matches,
        args.limit,
        if args.json { "items" } else { "lines" },
    );

    Ok(if args.json {
        render_list_symbols_json(
            &args.path,
            target == ListSymbolsTarget::File,
            &result.detected_filetypes,
            &result.server,
            &matches,
        )
    } else {
        render_symbol_names_text(&matches)
    })
}
