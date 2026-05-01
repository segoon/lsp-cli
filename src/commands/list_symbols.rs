use crate::cli::ListSymbolsArgs;
use crate::commands::symbol_query::{
    render_document_symbol_json, render_symbol_names_text, run_file_symbol_query, truncate_items,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &ListSymbolsArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_file_symbol_query(args, config)?;
    let matches = truncate_items(
        result.matches,
        args.limit,
        if args.json { "items" } else { "lines" },
    );

    Ok(if args.json {
        render_document_symbol_json(
            &args.file,
            &result.detected_filetypes,
            &result.server,
            &matches,
        )
    } else {
        render_symbol_names_text(&matches)
    })
}
