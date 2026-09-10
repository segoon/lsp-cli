#!/usr/bin/env bash
#
# Downloads the external runtimes needed by the real-server E2E tests (go, java, node, dotnet,
# zig, ruby) into a project-local .env/ directory, so contributors don't need to install them
# system-wide.
#
# Default versions mirror .github/workflows/ci.yml / e2e.yml (actions/setup-go,
# actions/setup-java, actions/setup-node, actions/setup-dotnet). Keep them in sync manually if
# CI's pins change; override per-tool via GO_VERSION / JAVA_VERSION / NODE_VERSION /
# DOTNET_CHANNEL / ZIG_VERSION / RUBY_VERSION.
#
# Each runtime's installer lives in scripts/download_dev_env/<name>.inc; add a new one there and
# source+call it below to add another language.
#
# Usage: scripts/download_dev_env.sh
# Then:  source activate.sh   (from the repo root)

set -euo pipefail

GO_VERSION="${GO_VERSION:-1.27.1}"
JAVA_VERSION="${JAVA_VERSION:-21}"
NODE_VERSION="${NODE_VERSION:-24}"
DOTNET_CHANNEL="${DOTNET_CHANNEL:-10.0}"
ZIG_VERSION="${ZIG_VERSION:-0.15.2}"
RUBY_VERSION="${RUBY_VERSION:-3.3.6}"

repo_root=$(git rev-parse --show-toplevel)
env_dir="$repo_root/.env"
bin_dir="$env_dir/bin"
inc_dir="$repo_root/scripts/download_dev_env"

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

detect_os() {
    case "$(uname -s)" in
        Linux) echo linux ;;
        *) echo "download-dev-env: unsupported OS '$(uname -s)' (only Linux is supported today)" >&2; exit 1 ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64) echo x64 ;;
        *) echo "download-dev-env: unsupported architecture '$(uname -m)' (only x86_64 is supported today)" >&2; exit 1 ;;
    esac
}

os=$(detect_os)
arch=$(detect_arch)

mkdir -p "$bin_dir"

version_stamp() {
    local tool_dir="$1"
    echo "$tool_dir/.lsp-cli-version"
}

is_up_to_date() {
    local tool_dir="$1"
    local version="$2"
    local stamp
    stamp=$(version_stamp "$tool_dir")
    [[ -f "$stamp" ]] && [[ "$(cat "$stamp")" == "$version" ]]
}

link() {
    local target="$1"
    local name="$2"
    ln -sf "$target" "$bin_dir/$name"
}

source "$inc_dir/go.inc"
source "$inc_dir/java.inc"
source "$inc_dir/node.inc"
source "$inc_dir/dotnet.inc"
source "$inc_dir/zig.inc"
source "$inc_dir/ruby.inc"

install_go
install_java
install_node
install_dotnet
install_zig
install_ruby

echo
echo "Dev env ready under $env_dir"
echo "Run: source activate.sh"
