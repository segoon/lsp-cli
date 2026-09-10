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

install_go() {
    local tool_dir="$env_dir/go"
    if is_up_to_date "$tool_dir" "$GO_VERSION"; then
        echo "go $GO_VERSION already installed, skipping"
        return
    fi
    echo "Installing go $GO_VERSION..."
    local archive="$tmp_dir/go.tar.gz"
    curl -fsSL -o "$archive" "https://go.dev/dl/go${GO_VERSION}.${os}-${arch/x64/amd64}.tar.gz"
    rm -rf "$tool_dir"
    mkdir -p "$tool_dir"
    tar -xzf "$archive" -C "$tool_dir" --strip-components=1
    echo "$GO_VERSION" > "$(version_stamp "$tool_dir")"
    link "$tool_dir/bin/go" go
    link "$tool_dir/bin/gofmt" gofmt
}

install_java() {
    local tool_dir="$env_dir/java"
    local api_url="https://api.adoptium.net/v3/binary/latest/${JAVA_VERSION}/ga/${os}/${arch}/jdk/hotspot/normal/eclipse"
    local resolved_version
    resolved_version=$(curl -fsSIL -o /dev/null -w '%{url_effective}' "$api_url")
    if is_up_to_date "$tool_dir" "$resolved_version"; then
        echo "java (Temurin $JAVA_VERSION) already installed, skipping"
        return
    fi
    echo "Installing java (Temurin $JAVA_VERSION)..."
    local archive="$tmp_dir/java.tar.gz"
    curl -fsSL -o "$archive" "$api_url"
    rm -rf "$tool_dir"
    mkdir -p "$tool_dir"
    tar -xzf "$archive" -C "$tool_dir" --strip-components=1
    echo "$resolved_version" > "$(version_stamp "$tool_dir")"
    link "$tool_dir/bin/java" java
    link "$tool_dir/bin/javac" javac
}

install_node() {
    local tool_dir="$env_dir/node"
    local index_url="https://nodejs.org/dist/index.json"
    local resolved_version
    resolved_version=$(curl -fsSL "$index_url" \
        | grep -o "\"version\":\"v${NODE_VERSION}\.[0-9]*\.[0-9]*\"" \
        | head -n1 \
        | sed -E 's/.*"v([0-9.]+)".*/\1/' || true)
    if [[ -z "$resolved_version" ]]; then
        echo "download-dev-env: could not resolve latest node ${NODE_VERSION}.x from $index_url" >&2
        exit 1
    fi
    if is_up_to_date "$tool_dir" "$resolved_version"; then
        echo "node $resolved_version already installed, skipping"
        return
    fi
    echo "Installing node $resolved_version..."
    local archive="$tmp_dir/node.tar.xz"
    curl -fsSL -o "$archive" "https://nodejs.org/dist/v${resolved_version}/node-v${resolved_version}-${os}-${arch}.tar.xz"
    rm -rf "$tool_dir"
    mkdir -p "$tool_dir"
    tar -xJf "$archive" -C "$tool_dir" --strip-components=1
    echo "$resolved_version" > "$(version_stamp "$tool_dir")"
    link "$tool_dir/bin/node" node
    link "$tool_dir/bin/npm" npm
    link "$tool_dir/bin/npx" npx
}

install_dotnet() {
    # dotnet-install.sh is already idempotent: it detects a matching SDK already present in
    # --install-dir and skips reinstalling it.
    local tool_dir="$env_dir/dotnet"
    local install_script="$tmp_dir/dotnet-install.sh"
    curl -fsSL -o "$install_script" "https://dot.net/v1/dotnet-install.sh"
    chmod +x "$install_script"
    echo "Installing dotnet (channel $DOTNET_CHANNEL)..."
    mkdir -p "$tool_dir"
    "$install_script" --channel "$DOTNET_CHANNEL" --install-dir "$tool_dir" --no-path
    link "$tool_dir/dotnet" dotnet
}

install_zig() {
    local tool_dir="$env_dir/zig"
    if is_up_to_date "$tool_dir" "$ZIG_VERSION"; then
        echo "zig $ZIG_VERSION already installed, skipping"
        return
    fi
    echo "Installing zig $ZIG_VERSION..."
    local archive="$tmp_dir/zig.tar.xz"
    curl -fsSL -o "$archive" "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-${os}-${ZIG_VERSION}.tar.xz"
    rm -rf "$tool_dir"
    mkdir -p "$tool_dir"
    tar -xJf "$archive" -C "$tool_dir" --strip-components=1
    echo "$ZIG_VERSION" > "$(version_stamp "$tool_dir")"
    link "$tool_dir/zig" zig
}

install_ruby() {
    # Prebuilt CRuby from ruby/ruby-builder (same source actions/setup-ruby uses), matched to
    # this host's Ubuntu release so the dynamically linked build actually runs.
    local tool_dir="$env_dir/ruby"
    if is_up_to_date "$tool_dir" "$RUBY_VERSION"; then
        echo "ruby $RUBY_VERSION already installed, skipping"
        return
    fi
    local ubuntu_codename
    ubuntu_codename=$(. /etc/os-release && echo "$VERSION_ID")
    echo "Installing ruby $RUBY_VERSION (ubuntu-${ubuntu_codename})..."
    local archive="$tmp_dir/ruby.tar.gz"
    curl -fsSL -o "$archive" "https://github.com/ruby/ruby-builder/releases/download/ruby-${RUBY_VERSION}/ruby-${RUBY_VERSION}-ubuntu-${ubuntu_codename}-x64.tar.gz"
    rm -rf "$tool_dir"
    mkdir -p "$tool_dir"
    tar -xzf "$archive" -C "$tool_dir" --strip-components=1
    # ruby-builder bakes its hosted-toolcache build path into every bin/ shebang and into the
    # binary's RUNPATH; rewrite the shebangs to this install's real ruby, and rely on
    # LD_LIBRARY_PATH (set by the E2E harness / activate.sh) rather than RUNPATH for libruby.
    for script in "$tool_dir"/bin/*; do
        [[ -f "$script" ]] || continue
        sed -i "1s|^#!.*/bin/ruby\$|#!${tool_dir}/bin/ruby|" "$script"
    done
    echo "$RUBY_VERSION" > "$(version_stamp "$tool_dir")"
    link "$tool_dir/bin/ruby" ruby
    link "$tool_dir/bin/gem" gem
    link "$tool_dir/bin/bundle" bundle
}

install_go
install_java
install_node
install_dotnet
install_zig
install_ruby

echo
echo "Dev env ready under $env_dir"
echo "Run: source activate.sh"
