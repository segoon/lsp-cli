// Q: is this file used at all?
use clap::{Arg, Command};
use clap_complete::{Shell, generate};
use std::io::Cursor;
use std::process::Command as ProcessCommand;

// Minimal reproducer for an upstream clap_complete bash bug with hyphenated
// binary names. Keep this file independent from lsp-cli-specific helpers so it
// can be copied into the clap_complete repository with minimal edits.

fn raw_bash_script(bin_name: &str) -> String {
    let mut command = Command::new("test-root").subcommand(
        Command::new("detect")
            .arg(Arg::new("path").default_value("."))
            .arg(
                Arg::new("lsp")
                    .long("lsp")
                    .value_parser(["clangd", "rust-analyzer"]),
            ),
    );
    let mut output = Cursor::new(Vec::new());
    generate(Shell::Bash, &mut command, bin_name, &mut output);
    String::from_utf8(output.into_inner()).expect("raw bash completion should be utf-8")
}

fn run_bash_completion(script: &str, invocation: &str) -> String {
    let output = ProcessCommand::new("/bin/bash")
        .arg("-lc")
        .arg(invocation)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;

            child
                .stdin
                .as_mut()
                .expect("stdin should be piped")
                .write_all(script.as_bytes())?;
            child.wait_with_output()
        })
        .expect("bash subprocess should run");

    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}

#[test]
fn raw_clap_complete_bash_output_uses_mismatched_detect_labels() {
    let script = raw_bash_script("my-cmd");

    assert!(script.contains("cmd=\"my__cmd__subcmd__detect\""));
    assert!(script.contains("my__subcmd__cmd__subcmd__detect)"));
}

#[test]
fn raw_clap_complete_bash_detect_lsp_completion_returns_no_candidates() {
    let stdout = run_bash_completion(
        &raw_bash_script("my-cmd"),
        "source /dev/stdin && COMP_WORDS=(my-cmd detect playground/c --lsp \"\") && COMP_CWORD=4 && COMPREPLY=() && _my-cmd my-cmd \"\" --lsp && printf 'count=%s\n' \"${#COMPREPLY[@]}\"",
    );

    assert_eq!(stdout, "count=0\n");
}

#[test]
fn raw_clap_complete_bash_output_uses_matching_detect_labels_without_dash() {
    let script = raw_bash_script("mycmd");

    assert!(script.contains("cmd=\"mycmd__subcmd__detect\""));
    assert!(script.contains("mycmd__subcmd__detect)"));
}

#[test]
fn raw_clap_complete_bash_detect_lsp_completion_returns_candidates_without_dash() {
    let stdout = run_bash_completion(
        &raw_bash_script("mycmd"),
        "source /dev/stdin && COMP_WORDS=(mycmd detect playground/c --lsp \"\") && COMP_CWORD=4 && COMPREPLY=() && _mycmd mycmd \"\" --lsp && printf '%s\n' \"${COMPREPLY[@]}\"",
    );

    assert!(stdout.contains("clangd"));
    assert!(stdout.contains("rust-analyzer"));
}
