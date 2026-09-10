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
