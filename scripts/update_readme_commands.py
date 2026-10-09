#!/usr/bin/env python3

import argparse
import os
import subprocess
import sys
from pathlib import Path


BEGIN_MARKER = "<!-- BEGIN GENERATED COMMANDS -->"
END_MARKER = "<!-- END GENERATED COMMANDS -->"
IGNORED_COMMANDS = [
    'commands',
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Build lsp-cli, capture `--help` output for every top-level subcommand, "
            "and update docs/COMMANDS.md."
        )
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check",
        action="store_true",
        help="Exit with an error if docs/COMMANDS.md is out of date instead of rewriting it.",
    )
    mode.add_argument(
        "--write",
        action="store_true",
        help="Rewrite docs/COMMANDS.md with the generated command reference. This is the default.",
    )
    return parser.parse_args()


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def binary_path(root: Path) -> Path:
    exe_name = "lsp-cli.exe" if os.name == "nt" else "lsp-cli"
    path = root / "target" / "debug" / exe_name
    if not path.exists():
        fail(f"built binary not found at {path}")
    return path


def run_cargo_build(root: Path) -> None:
    command = ["cargo", "build", "--locked", "-q"]
    result = subprocess.run(command, cwd=root, capture_output=True, text=True)
    if result.returncode != 0:
        fail_command(command, result, "failed to build lsp-cli")


def run_command(root: Path, command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=root, capture_output=True, text=True)


def run_commands_subcommand(root: Path, binary: Path) -> list[str]:
    invocation = [str(binary), "commands"]
    result = run_command(root, invocation)
    if result.returncode != 0:
        fail_command(invocation, result, "`lsp-cli commands` failed")

    commands = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    if not commands:
        fail("`lsp-cli commands` returned no subcommands")
    return commands


def capture_help(root: Path, binary: Path, extra_args: list[str]) -> str:
    invocation = [str(binary), *extra_args, "--help"]
    result = run_command(root, invocation)
    output = choose_help_output(result)
    if output is None:
        label = " ".join(["lsp-cli", *extra_args, "--help"]).strip()
        fail_command(invocation, result, f"`{label}` did not produce help output")
    return output.rstrip()


def choose_help_output(result: subprocess.CompletedProcess[str]) -> str | None:
    stdout = result.stdout.strip()
    stderr = result.stderr.strip()
    if result.returncode not in (0, 2):
        return None
    if stdout and stderr:
        return f"{stdout}\n{stderr}"
    if stdout:
        return stdout
    if stderr:
        return stderr
    return None


def render_block(command: str, output: str) -> str:
    return f"```text\n$ {command}\n{output}\n```"


def render_generated_section(root: Path, binary: Path) -> str:
    blocks = [render_block("lsp-cli --help", capture_help(root, binary, []))]
    for command in run_commands_subcommand(root, binary):
        if command in IGNORED_COMMANDS:
            continue
        blocks.append(
            render_block(
                f"lsp-cli {command} --help",
                capture_help(root, binary, [command]),
            )
        )
    return "\n\n".join(blocks)


def replace_generated_section(document: str, generated: str) -> str:
    begin_count = document.count(BEGIN_MARKER)
    end_count = document.count(END_MARKER)
    if begin_count != 1 or end_count != 1:
        fail(
            "docs/COMMANDS.md must contain exactly one generated commands marker pair "
            f"({BEGIN_MARKER} ... {END_MARKER})"
        )

    begin = document.index(BEGIN_MARKER) + len(BEGIN_MARKER)
    end = document.index(END_MARKER)
    if begin > end:
        fail("docs/COMMANDS.md generated commands markers are in the wrong order")

    body = f"\n{generated}\n"
    return document[:begin] + body + document[end:]


def fail_command(command: list[str], result: subprocess.CompletedProcess[str], message: str) -> None:
    rendered_command = " ".join(command)
    details = [f"{message}: {rendered_command}"]
    if result.stdout.strip():
        details.append(f"stdout:\n{result.stdout.rstrip()}")
    if result.stderr.strip():
        details.append(f"stderr:\n{result.stderr.rstrip()}")
    fail("\n\n".join(details))


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def main() -> int:
    args = parse_args()
    root = repo_root()
    document_path = root / "docs/COMMANDS.md"
    run_cargo_build(root)
    binary = binary_path(root)

    generated = render_generated_section(root, binary)
    original = document_path.read_text(encoding="utf-8")
    updated = replace_generated_section(original, generated)

    if args.check:
        if updated != original:
            print(
                "docs/COMMANDS.md Commands and options section is out of date; "
                "run scripts/update_readme_commands.py",
                file=sys.stderr,
            )
            return 1
        return 0

    document_path.write_text(updated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
