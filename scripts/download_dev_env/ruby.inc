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
