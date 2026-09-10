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
