use std::collections::BTreeMap;
use std::ffi::OsString;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::time::Instant;

use super::*;

fn context() -> E2eContext {
    E2eContext::new().expect("E2E context should initialize")
}

fn run_shell(context: &E2eContext, script: &str, deadline: Duration) -> Result<E2eOutput, String> {
    context.run_test_program("/bin/sh", &["-c", script], deadline)
}

#[cfg(target_os = "linux")]
fn assert_recorded_process_is_gone(pid_file: &Path) {
    let pid = fs::read_to_string(pid_file).expect("descendant PID should be recorded");
    let process = Path::new("/proc").join(&pid);
    let reaping_deadline = Instant::now() + Duration::from_secs(1);
    while process.exists() && Instant::now() < reaping_deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process.exists(),
        "descendant process {pid} survived group cleanup"
    );
}

#[test]
fn command_isolated_from_ambient_process_state() {
    let context = context();
    let command = context.command();
    let actual = command
        .get_envs()
        .map(|(name, value)| (name.to_os_string(), value.map(OsString::from)))
        .collect::<BTreeMap<_, _>>();
    let expected = [
        (
            "CARGO_TARGET_DIR",
            context.build_dir.as_os_str().to_os_string(),
        ),
        ("HOME", context.home.as_os_str().to_os_string()),
        ("GOFLAGS", OsString::from("-modcacherw")),
        (
            "GRADLE_USER_HOME",
            context.home.join(".gradle").into_os_string(),
        ),
        (
            "JAVA_TOOL_OPTIONS",
            OsString::from(format!(
                "-Djava.io.tmpdir={} -Duser.home={}",
                context.temp_dir.display(),
                context.home.display()
            )),
        ),
        ("LANG", OsString::from("C")),
        ("LC_ALL", OsString::from("C")),
        ("LSP_DATA", context.data_dir.as_os_str().to_os_string()),
        ("PATH", context.bin_dir.as_os_str().to_os_string()),
        ("TMPDIR", context.temp_dir.as_os_str().to_os_string()),
        ("TZ", OsString::from("UTC")),
        (
            "XDG_CONFIG_HOME",
            context.config_home.as_os_str().to_os_string(),
        ),
        (
            "XDG_RUNTIME_DIR",
            context.runtime_dir.as_os_str().to_os_string(),
        ),
    ]
    .into_iter()
    .map(|(name, value)| (OsString::from(name), Some(value)))
    .collect::<BTreeMap<_, _>>();

    assert_eq!(actual, expected);
    assert_eq!(command.get_current_dir(), Some(context.workspace.as_path()));
    assert!(!context.runtime_dir.starts_with(context._sandbox.path()));
    assert!(!context.temp_dir.starts_with("/tmp"));
}

#[cfg(unix)]
#[test]
fn runtime_directory_has_room_for_daemon_socket_name() {
    use std::os::unix::net::UnixListener;

    let context = context();
    let daemon_root = context.runtime_dir.join("lsp-cli");
    fs::create_dir(&daemon_root).expect("daemon root should be created");
    let socket_path = daemon_root.join(format!("{}-{}.sock", "s".repeat(32), "f".repeat(24)));
    let _listener = UnixListener::bind(&socket_path).unwrap_or_else(|error| {
        panic!(
            "E2E runtime path {} cannot hold a daemon socket: {error}",
            socket_path.display()
        )
    });
}

#[test]
fn captures_large_stdout_and_stderr_without_deadlock() {
    let context = context();
    let output = run_shell(
        &context,
        "i=0; while [ \"$i\" -lt 100000 ]; do printf o; printf e >&2; i=$((i + 1)); done",
        Duration::from_secs(5),
    )
    .expect("output fixture should finish");

    assert_eq!(output.process.stdout().len(), 100_000);
    assert_eq!(output.process.stderr().len(), 100_000);
}

#[test]
fn deadline_kills_the_command_process_group() {
    let context = context();
    let pid_file = context.workspace.join("descendant.pid");
    let script = format!(
        "/bin/sleep 30 & child=$!; printf '%s' \"$child\" > {}; wait",
        pid_file.display()
    );
    let diagnostic = run_shell(&context, &script, Duration::from_millis(100))
        .err()
        .expect("stalled fixture should exceed its deadline");

    assert!(diagnostic.contains("process exceeded its deadline"));
    assert!(diagnostic.contains("process group killed and reaped"));
    #[cfg(target_os = "linux")]
    assert_recorded_process_is_gone(&pid_file);
}

#[test]
fn deadline_includes_output_pipes_held_by_descendants() {
    let context = context();
    let pid_file = context.workspace.join("pipe-holder.pid");
    let script = format!(
        "/bin/sleep 30 & child=$!; printf '%s' \"$child\" > {}",
        pid_file.display()
    );
    let diagnostic = run_shell(&context, &script, Duration::from_millis(100))
        .err()
        .expect("inherited pipe should keep the process group beyond its deadline");

    assert!(diagnostic.contains("remained open after the command deadline"));
    assert!(diagnostic.contains("process group killed and reaped"));
    #[cfg(target_os = "linux")]
    assert_recorded_process_is_gone(&pid_file);
}

#[test]
fn parses_json_output_into_requested_type() {
    let context = context();
    let output = run_shell(
        &context,
        "printf '%s' '{\"answer\":42}'",
        Duration::from_secs(1),
    )
    .expect("JSON fixture should finish");
    let value: serde_json::Value = output.json();

    assert_eq!(value, serde_json::json!({"answer": 42}));
}

#[test]
fn invalid_json_reports_command_and_captured_output() {
    let context = context();
    let output = run_shell(&context, "printf not-json", Duration::from_secs(1))
        .expect("invalid JSON fixture should finish");
    let diagnostic = output
        .try_json::<serde_json::Value>()
        .expect_err("invalid JSON should be rejected");

    assert!(diagnostic.contains("stdout is not valid JSON"));
    assert!(diagnostic.contains("command: \"/bin/sh\" \"-c\""));
    assert!(diagnostic.contains("not-json"));
}

#[test]
fn failed_command_diagnostic_includes_execution_context_and_output() {
    let context = context();
    let output = run_shell(
        &context,
        "printf stdout-marker; printf stderr-marker >&2; exit 7",
        Duration::from_secs(1),
    )
    .expect("failure fixture should finish");
    let diagnostic = output.diagnostic("fixture failed");

    assert!(diagnostic.contains("fixture failed"));
    assert!(diagnostic.contains("command: \"/bin/sh\" \"-c\""));
    assert!(diagnostic.contains(&format!(
        "working directory: {}",
        context.workspace.display()
    )));
    assert!(diagnostic.contains("status: exit status: 7"));
    assert!(diagnostic.contains("stdout-marker"));
    assert!(diagnostic.contains("stderr-marker"));
    assert!(diagnostic.contains("runtime state:"));
}
