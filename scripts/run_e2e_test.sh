#!/usr/bin/env bash

set -euo pipefail

if (( $# != 1 )); then
    echo "usage: $0 TEST_FILTER" >&2
    exit 2
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
case "${E2E_AUTO_DOWNLOAD:-1}" in
    1)
        "$repo_root/scripts/ensure_e2e_dependencies.sh"
        export PATH="$repo_root/.env/bin:$PATH"
        export DOTNET_ROOT="$repo_root/.env/dotnet"
        if [[ -d "$repo_root/.env/ruby/lib" ]]; then
            export LD_LIBRARY_PATH="$repo_root/.env/ruby/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            rubylib_entries=()
            for dir in "$repo_root"/.env/ruby/lib/ruby/*/; do
                [[ -d "$dir" ]] || continue
                rubylib_entries+=("${dir%/}")
                for arch_dir in "$dir"*linux*/; do
                    [[ -d "$arch_dir" ]] && rubylib_entries+=("${arch_dir%/}")
                done
            done
            if (( ${#rubylib_entries[@]} > 0 )); then
                rubylib_joined=$(IFS=:; echo "${rubylib_entries[*]}")
                export RUBYLIB="$rubylib_joined${RUBYLIB:+:$RUBYLIB}"
            fi
            unset rubylib_entries rubylib_joined dir arch_dir
        fi
        ;;
    0) ;;
    *)
        echo "E2E_AUTO_DOWNLOAD must be 0 or 1" >&2
        exit 2
        ;;
esac

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
