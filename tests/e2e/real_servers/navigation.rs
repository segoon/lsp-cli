use crate::harness::E2eContext;
use crate::real_server_support::CaseDeadline;
use serde_json::Value;

pub(super) fn run(
    context: &E2eContext,
    server: &str,
    deadline: &CaseDeadline,
) -> Result<(), String> {
    // Use usage sites for value navigation, and callable identifiers for hierarchy.
    for (command, file, needle) in [
        ("definition", "src/main.rs", "sample_order"),
        ("declaration", "src/main.rs", "sample_order"),
        ("references", "src/order.rs", "sample_order"),
        ("implementation", "src/order.rs", "OrderTotaling"),
        ("type-definition", "src/main.rs", "order ="),
        ("callers", "src/order.rs", "sample_order"),
        ("callees", "src/report/mod.rs", "format_order"),
    ] {
        let source = std::fs::read_to_string(context.workspace().join(file))
            .map_err(|error| error.to_string())?;
        let Some(offset) = source.find(needle) else {
            return Err(format!(
                "missing navigation fixture text {needle:?} in {file}"
            ));
        };
        let prefix = source
            .get(..offset)
            .expect("find returns a character boundary");
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count()
            + 1;
        let location = format!("{file}:{line}:{column}");
        for assertion in [
            None,
            Some(needle.split_whitespace().next().expect("non-empty needle")),
        ] {
            let mut args = vec![
                command,
                "--at",
                &location,
                "--lang",
                "rust",
                "--lsp",
                server,
                "--json",
                "--no-download",
                "--no-detach",
                "--timeout",
                "15",
            ];
            if let Some(assertion) = assertion {
                args.push(assertion);
            }
            let output = context.try_run_with_deadline(&args, deadline.remaining()?)?;
            output.ensure_success()?;
            let value: Value = output.try_json()?;
            let Some(matches) = value.get("matches").and_then(Value::as_array) else {
                return Err(format!("{command} --at returned no matches array"));
            };
            if matches.is_empty() {
                return Err(format!("{command} --at {location} returned no matches"));
            }
            if value.get("at").and_then(Value::as_str) != Some(location.as_str()) {
                return Err(format!("{command} did not retain its position target"));
            }
        }
    }
    Ok(())
}
