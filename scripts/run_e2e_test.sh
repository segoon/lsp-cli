#!/usr/bin/env bash

set -euo pipefail

if (( $# != 1 )); then
    echo "usage: $0 TEST_FILTER" >&2
    exit 2
fi

log_file=$(mktemp)
trap 'rm -f "$log_file"' EXIT

set +e
cargo test --locked --test e2e "$1" -- --ignored --nocapture --test-threads=1 2>&1 \
    | tee "$log_file"
status=${PIPESTATUS[0]}
set -e

if (( status != 0 )); then
    mapfile -t failed_cases < <(
        sed -nE \
            's/^E2E (lifecycle |capabilities case |case |provisioning )([^[:space:]]+) failed:$/\2/p' \
            "$log_file" \
            | sort -u
    )
    echo
    echo "E2E failed case IDs:"
    if (( ${#failed_cases[@]} == 0 )); then
        echo "- unavailable; inspect the diagnostics above"
    else
        printf -- '- %s\n' "${failed_cases[@]}"
    fi
fi

exit "$status"
