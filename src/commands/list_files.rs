use crate::cli::ListFilesArgs;
use crate::commands::symbol_query::{
    render_file_list_json, render_paths_text, run_list_files_query, truncate_items,
};
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(args: &ListFilesArgs, config: &ConfigStore) -> Result<String> {
    let query = &args.query;
    let result = run_list_files_query(query, config)?;

    let files = truncate_items(
        result.files,
        query.limit,
        if query.json { "items" } else { "lines" },
    );

    Ok(if query.json {
        render_file_list_json(
            &query.directory,
            &result.detected_filetypes,
            &result.server,
            &files,
        )
    } else {
        render_paths_text(&files)
    })
}
