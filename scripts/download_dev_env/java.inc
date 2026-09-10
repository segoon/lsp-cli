install_java() {
    local tool_dir="$env_dir/java"
    local api_url="https://api.adoptium.net/v3/binary/latest/${JAVA_VERSION}/ga/${os}/${arch}/jdk/hotspot/normal/eclipse"
    local resolved_version
    resolved_version=$(curl -fsSIL -o /dev/null -w '%{url_effective}' "$api_url")
    if is_up_to_date "$tool_dir" "$resolved_version"; then
        echo "java (Temurin $JAVA_VERSION) already installed, skipping"
        return
    fi
    echo "Installing java (Temurin $JAVA_VERSION)..."
    local archive="$tmp_dir/java.tar.gz"
    curl -fsSL -o "$archive" "$api_url"
    rm -rf "$tool_dir"
    mkdir -p "$tool_dir"
    tar -xzf "$archive" -C "$tool_dir" --strip-components=1
    echo "$resolved_version" > "$(version_stamp "$tool_dir")"
    link "$tool_dir/bin/java" java
    link "$tool_dir/bin/javac" javac
}
