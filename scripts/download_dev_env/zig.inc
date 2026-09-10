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
