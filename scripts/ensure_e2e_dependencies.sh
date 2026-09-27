#!/usr/bin/env bash

# Installs the pinned third-party runtimes used by real-server E2E tests. Rust and basic
# operating-system tools are prerequisites because Cargo and this script must run first.

set -euo pipefail

GO_VERSION=1.27.1
JAVA_VERSION=21.0.12.1+1
NODE_VERSION=24.21.0
DOTNET_VERSION=10.0.401
ZIG_VERSION=0.15.2
RUBY_VERSION=3.3.6

GO_SHA256=63d339f0da5ab53635a56f2490a7984dfe12dfcff22ad749f63edaf590168445
JAVA_SHA256=ce79869e1307ed8ee1e2baa86a412b1eb5b75d10a01006d788a6f968bcfaee94
NODE_SHA256=fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6
DOTNET_SHA512=51c8b999af9e8dd9998c9edc5944e19a90788862068acd38694e098889054ce8c23d4f0c5cccfa16bf187d044562359e5ee69a9f8ad0bbe913ba90311fbce25b
ZIG_SHA256=02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239
RUBY_UBUNTU_2204_SHA256=d8bafa50f7190148473ad0d4b14b4dc3da60be7a490887092257ab2dc9b8fe5b
RUBY_UBUNTU_2404_SHA256=e7317b23328584cfa46c08c0860048e3198617a1c03f3a92783a2820738b7035

require_program() {
    local name="$1"
    if ! command -v "$name" >/dev/null 2>&1; then
        echo "E2E dependency setup requires '$name', but it is not available on PATH" >&2
        return 1
    fi
}

version_stamp() {
    echo "$1/.lsp-cli-version"
}

is_up_to_date() {
    local tool_dir="$1"
    local version="$2"
    local validator="$3"
    local stamp
    stamp=$(version_stamp "$tool_dir")
    [[ -f "$stamp" ]] && [[ "$(<"$stamp")" == "$version" ]] && "$validator" "$tool_dir"
}

record_version() {
    local tool_dir="$1"
    local version="$2"
    echo "$version" > "$(version_stamp "$tool_dir")"
}

download_verified() {
    local url="$1"
    local checksum="$2"
    local algorithm="$3"
    local destination="$4"
    echo "Downloading $url"
    curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$destination" "$url"
    verify_checksum "$destination" "$checksum" "$algorithm" || {
        echo "E2E dependency download from $url failed its $algorithm checksum" >&2
        return 1
    }
}

verify_checksum() {
    local path="$1"
    local checksum="$2"
    local algorithm="$3"
    printf '%s  %s\n' "$checksum" "$path" | "${algorithm}sum" --check --status
}

new_stage() {
    local name="$1"
    local stage="$tmp_dir/install-$name"
    mkdir -p "$stage"
    echo "$stage"
}

promote_stage() {
    local stage="$1"
    local destination="$2"
    local backup="$tmp_dir/backup-$(basename "$destination")"
    if [[ -e "$destination" ]]; then
        mv "$destination" "$backup"
    fi
    if mv "$stage" "$destination"; then
        # Both paths are generated below .env; removing the superseded cache is intentional.
        [[ ! -e "$backup" ]] || rm -rf -- "$backup"
    else
        [[ ! -e "$backup" ]] || mv "$backup" "$destination"
        return 1
    fi
}

link_program() {
    local target="$1"
    local name="$2"
    ln -sfn "$target" "$bin_dir/$name"
}

acquire_lock() {
    local attempts=0
    while ! mkdir "$lock_dir" 2>/dev/null; do
        local owner=""
        [[ ! -f "$lock_dir/pid" ]] || owner=$(<"$lock_dir/pid")
        if [[ "$owner" =~ ^[0-9]+$ ]] && ! kill -0 "$owner" 2>/dev/null; then
            rm -f -- "$lock_dir/pid"
            rmdir "$lock_dir" 2>/dev/null || true
            continue
        fi
        if (( attempts >= 600 )); then
            echo "timed out waiting for another E2E dependency setup to finish" >&2
            return 1
        fi
        sleep 0.5
        attempts=$((attempts + 1))
    done
    echo "$$" > "$lock_dir/pid"
    lock_owned=1
}

cleanup() {
    local status=$?
    # tmp_dir and lock_dir are exact paths created by this process under the repository cache.
    [[ -z "${tmp_dir:-}" || ! -d "$tmp_dir" ]] || rm -rf -- "$tmp_dir"
    if [[ "${lock_owned:-0}" == 1 ]]; then
        rm -f -- "$lock_dir/pid"
        rmdir "$lock_dir" 2>/dev/null || true
    fi
    return "$status"
}

main() {
    for program in curl sha256sum sha512sum tar xz; do
        require_program "$program"
    done
    if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
        echo "automatic E2E dependency setup supports Linux x86-64 only; this host reports $(uname -s) $(uname -m)" >&2
        return 1
    fi

    local repo_root
    repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
    env_dir="$repo_root/.env"
    bin_dir="$env_dir/bin"
    lock_dir="$env_dir/.e2e-dependencies.lock"
    lock_owned=0
    mkdir -p "$bin_dir"
    acquire_lock
    trap cleanup EXIT
    tmp_dir=$(mktemp -d "$env_dir/.e2e-dependencies.XXXXXX")

    local inc_dir="$repo_root/scripts/e2e_dependencies"
    source "$inc_dir/go.inc"
    source "$inc_dir/java.inc"
    source "$inc_dir/node.inc"
    source "$inc_dir/dotnet.inc"
    source "$inc_dir/zig.inc"
    source "$inc_dir/ruby.inc"

    # Ruby is the distribution-specific dependency; reject unsupported hosts before downloading
    # any of the otherwise portable Linux archives.
    ruby_ubuntu_release >/dev/null
    install_go
    install_java
    install_node
    install_dotnet
    install_zig
    install_ruby
    echo "E2E dependencies are ready under $env_dir"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
