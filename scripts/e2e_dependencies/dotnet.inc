validate_dotnet() {
    [[ "$(DOTNET_ROOT="$1" "$1/dotnet" --version 2>/dev/null)" == "$DOTNET_VERSION" ]]
}

install_dotnet() {
    local tool_dir="$env_dir/dotnet"
    if ! is_up_to_date "$tool_dir" "$DOTNET_VERSION" validate_dotnet; then
        echo "Installing .NET SDK $DOTNET_VERSION"
        local archive="$tmp_dir/dotnet.tar.gz" stage
        local url="https://builds.dotnet.microsoft.com/dotnet/Sdk/${DOTNET_VERSION}/dotnet-sdk-${DOTNET_VERSION}-linux-x64.tar.gz"
        download_verified "$url" "$DOTNET_SHA512" sha512 "$archive"
        stage=$(new_stage dotnet)
        tar -xzf "$archive" -C "$stage"
        record_version "$stage" "$DOTNET_VERSION"
        validate_dotnet "$stage" || { echo "downloaded .NET SDK $DOTNET_VERSION cannot run" >&2; return 1; }
        promote_stage "$stage" "$tool_dir"
    fi
    link_program "$tool_dir/dotnet" dotnet
}
