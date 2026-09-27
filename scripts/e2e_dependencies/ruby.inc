ruby_ubuntu_release() {
    local release_file="${E2E_OS_RELEASE_FILE:-/etc/os-release}"
    local ID="" VERSION_ID=""
    # os-release is a shell-compatible assignment file supplied by the operating system.
    source "$release_file"
    if [[ "$ID" != ubuntu || ! "$VERSION_ID" =~ ^(22\.04|24\.04)$ ]]; then
        echo "automatic E2E Ruby setup supports Ubuntu 22.04 and 24.04 x86-64; this host reports ${ID:-unknown} ${VERSION_ID:-unknown}" >&2
        return 1
    fi
    echo "$VERSION_ID"
}

validate_ruby() {
    LD_LIBRARY_PATH="$1/lib" "$1/bin/ruby" --version 2>/dev/null | grep -Fq "ruby $RUBY_VERSION"
}

install_ruby() {
    local ubuntu_release checksum
    ubuntu_release=$(ruby_ubuntu_release)
    case "$ubuntu_release" in
        22.04) checksum="$RUBY_UBUNTU_2204_SHA256" ;;
        24.04) checksum="$RUBY_UBUNTU_2404_SHA256" ;;
    esac
    local tool_dir="$env_dir/ruby"
    if ! is_up_to_date "$tool_dir" "$RUBY_VERSION-ubuntu-$ubuntu_release" validate_ruby; then
        echo "Installing Ruby $RUBY_VERSION for Ubuntu $ubuntu_release"
        local archive="$tmp_dir/ruby.tar.gz" stage
        local name="ruby-${RUBY_VERSION}-ubuntu-${ubuntu_release}-x64.tar.gz"
        download_verified "https://github.com/ruby/ruby-builder/releases/download/ruby-${RUBY_VERSION}/${name}" "$checksum" sha256 "$archive"
        stage=$(new_stage ruby)
        tar -xzf "$archive" -C "$stage" --strip-components=1
        # ruby-builder embeds its hosted-toolcache interpreter path in script shebangs.
        for script in "$stage"/bin/*; do
            [[ -f "$script" ]] || continue
            sed -i "1s|^#!.*/bin/ruby\$|#!${tool_dir}/bin/ruby|" "$script"
        done
        record_version "$stage" "$RUBY_VERSION-ubuntu-$ubuntu_release"
        validate_ruby "$stage" || { echo "downloaded Ruby $RUBY_VERSION cannot run" >&2; return 1; }
        promote_stage "$stage" "$tool_dir"
    fi
    link_program "$tool_dir/bin/ruby" ruby
    link_program "$tool_dir/bin/gem" gem
    link_program "$tool_dir/bin/bundle" bundle
}
