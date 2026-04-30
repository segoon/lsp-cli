use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Detect(DetectArgs),
    Grep(GrepArgs),
}

#[derive(Debug, Eq, PartialEq)]
pub struct DetectArgs {
    pub path: PathBuf,
    pub json: bool,
    pub quiet: bool,
    pub debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GrepArgs {
    pub pattern: String,
    pub directory: PathBuf,
    pub lsp: Option<String>,
    pub json: bool,
    pub debug: bool,
}

pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(usage().to_string());
    };

    match command.as_str() {
        "detect" => parse_detect(args),
        "grep" => parse_grep(args),
        flag if flag.starts_with('-') => Err(format!("unknown flag: {flag}\n{}", usage())),
        _ => Err(format!("unknown subcommand: {command}\n{}", usage())),
    }
}

pub fn usage() -> &'static str {
    "usage: lsp-cli detect [PATH] [--json] [-q] [--debug]\n       lsp-cli grep PATTERN DIRECTORY [--json] [--lsp SERVER] [--debug]"
}

fn parse_detect<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut path = None;
    let mut json = false;
    let mut quiet = false;
    let mut debug = false;

    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            "-q" => quiet = true,
            "--debug" => debug = true,
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag: {flag}\n{}", usage()));
            }
            _ => {
                if path.replace(PathBuf::from(arg)).is_some() {
                    return Err(usage().to_string());
                }
            }
        }
    }

    Ok(Command::Detect(DetectArgs {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        json,
        quiet,
        debug,
    }))
}

fn parse_grep<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut positionals = Vec::new();
    let mut json = false;
    let mut lsp = None;
    let mut debug = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--debug" => debug = true,
            "--lsp" => {
                let server = args.next().ok_or_else(|| {
                    format!("missing value for --lsp\n{}", usage())
                })?;
                lsp = Some(server);
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag: {flag}\n{}", usage()));
            }
            _ => positionals.push(arg),
        }
    }

    if positionals.len() != 2 {
        return Err(usage().to_string());
    }

    Ok(Command::Grep(GrepArgs {
        pattern: positionals.remove(0),
        directory: PathBuf::from(positionals.remove(0)),
        lsp,
        json,
        debug,
    }))
}

#[cfg(test)]
mod tests {
    use super::{Command, DetectArgs, GrepArgs, parse_args, usage};
    use std::path::PathBuf;

    #[test]
    fn parses_detect_defaults() {
        assert_eq!(
            parse_args(vec!["detect".to_string()]).expect("detect should parse"),
            Command::Detect(DetectArgs {
                path: PathBuf::from("."),
                json: false,
                quiet: false,
                debug: false,
            })
        );
    }

    #[test]
    fn parses_detect_flags_and_path() {
        assert_eq!(
            parse_args(vec![
                "detect".to_string(),
                "src".to_string(),
                "--json".to_string(),
                "-q".to_string(),
            ])
            .expect("detect should parse"),
            Command::Detect(DetectArgs {
                path: PathBuf::from("src"),
                json: true,
                quiet: true,
                debug: false,
            })
        );
    }

    #[test]
    fn parses_grep_arguments() {
        assert_eq!(
            parse_args(vec![
                "grep".to_string(),
                "needle".to_string(),
                "workspace".to_string(),
                "--json".to_string(),
                "--lsp".to_string(),
                "clangd".to_string(),
                "--debug".to_string(),
            ])
            .expect("grep should parse"),
            Command::Grep(GrepArgs {
                pattern: "needle".to_string(),
                directory: PathBuf::from("workspace"),
                lsp: Some("clangd".to_string()),
                json: true,
                debug: true,
            })
        );
    }

    #[test]
    fn rejects_missing_lsp_value() {
        assert_eq!(
            parse_args(vec!["grep".to_string(), "needle".to_string(), "workspace".to_string(), "--lsp".to_string()]),
            Err(format!("missing value for --lsp\n{}", usage()))
        );
    }

    #[test]
    fn rejects_missing_subcommand() {
        assert_eq!(parse_args(Vec::<String>::new()), Err(usage().to_string()));
    }
}
