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
