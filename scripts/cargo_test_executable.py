#!/usr/bin/env python3
"""Print one integration-test executable from Cargo's JSON build output."""

import json
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: cargo_test_executable.py TARGET", file=sys.stderr)
        return 2

    target = sys.argv[1]
    executables = set()
    for line in sys.stdin:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        artifact_target = message.get("target", {})
        executable = message.get("executable")
        if (
            message.get("reason") == "compiler-artifact"
            and artifact_target.get("name") == target
            and message.get("profile", {}).get("test")
            and executable
        ):
            executables.add(executable)

    if len(executables) != 1:
        print(
            f"expected one Cargo test executable for {target!r}, found {len(executables)}",
            file=sys.stderr,
        )
        return 1
    print(executables.pop())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
