validate_node() {
    [[ "$("$1/bin/node" --version 2>/dev/null)" == "v$NODE_VERSION" ]]
}

install_node() {
    local tool_dir="$env_dir/node"
    if ! is_up_to_date "$tool_dir" "$NODE_VERSION" validate_node; then
        echo "Installing Node.js $NODE_VERSION"
        local archive="$tmp_dir/node.tar.xz" stage
        local url="https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz"
        download_verified "$url" "$NODE_SHA256" sha256 "$archive"
        stage=$(new_stage node)
        tar -xJf "$archive" -C "$stage" --strip-components=1
        record_version "$stage" "$NODE_VERSION"
        validate_node "$stage" || { echo "downloaded Node.js $NODE_VERSION cannot run" >&2; return 1; }
        promote_stage "$stage" "$tool_dir"
    fi
    link_program "$tool_dir/bin/node" node
    link_program "$tool_dir/bin/npm" npm
    link_program "$tool_dir/bin/npx" npx
}
