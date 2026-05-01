use crate::cli::ListFilesArgs;
use crate::commands::symbol_query::{
    render_file_list_json, render_paths_text, run_list_files_query, truncate_items,
};
use crate::config::ConfigStore;

pub(super) fn run(args: &ListFilesArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_list_files_query(&args.query, config)?;
    let files = truncate_items(
        result.files,
        args.query.limit,
        if args.query.json { "items" } else { "lines" },
    );

    Ok(if args.query.json {
        render_file_list_json(
            &args.query.directory,
            &result.detected_filetypes,
            &result.server,
            &files,
        )
    } else {
        render_paths_text(&files)
    })
}
