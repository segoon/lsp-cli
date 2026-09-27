validate_zig() {
    [[ "$("$1/zig" version 2>/dev/null)" == "$ZIG_VERSION" ]]
}

install_zig() {
    local tool_dir="$env_dir/zig"
    if ! is_up_to_date "$tool_dir" "$ZIG_VERSION" validate_zig; then
        echo "Installing Zig $ZIG_VERSION"
        local archive="$tmp_dir/zig.tar.xz" stage
        local url="https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz"
        download_verified "$url" "$ZIG_SHA256" sha256 "$archive"
        stage=$(new_stage zig)
        tar -xJf "$archive" -C "$stage" --strip-components=1
        record_version "$stage" "$ZIG_VERSION"
        validate_zig "$stage" || { echo "downloaded Zig $ZIG_VERSION cannot run" >&2; return 1; }
        promote_stage "$stage" "$tool_dir"
    fi
    link_program "$tool_dir/zig" zig
}
