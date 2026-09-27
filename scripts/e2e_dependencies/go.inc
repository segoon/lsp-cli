validate_go() {
    "$1/bin/go" version 2>/dev/null | grep -Fq "go$GO_VERSION"
}

install_go() {
    local tool_dir="$env_dir/go"
    if ! is_up_to_date "$tool_dir" "$GO_VERSION" validate_go; then
        echo "Installing Go $GO_VERSION"
        local archive="$tmp_dir/go.tar.gz" stage
        download_verified "https://go.dev/dl/go${GO_VERSION}.linux-amd64.tar.gz" "$GO_SHA256" sha256 "$archive"
        stage=$(new_stage go)
        tar -xzf "$archive" -C "$stage" --strip-components=1
        record_version "$stage" "$GO_VERSION"
        validate_go "$stage" || { echo "downloaded Go $GO_VERSION cannot run" >&2; return 1; }
        promote_stage "$stage" "$tool_dir"
    fi
    link_program "$tool_dir/bin/go" go
    link_program "$tool_dir/bin/gofmt" gofmt
}
