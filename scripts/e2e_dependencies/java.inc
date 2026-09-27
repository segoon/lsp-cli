validate_java() {
    "$1/bin/java" -version 2>&1 | grep -Fq '21.0.12.1'
}

install_java() {
    local tool_dir="$env_dir/java"
    if ! is_up_to_date "$tool_dir" "$JAVA_VERSION" validate_java; then
        echo "Installing Temurin Java $JAVA_VERSION"
        local archive="$tmp_dir/java.tar.gz" stage
        local url="https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jdk_x64_linux_hotspot_21.0.12.1_1.tar.gz"
        download_verified "$url" "$JAVA_SHA256" sha256 "$archive"
        stage=$(new_stage java)
        tar -xzf "$archive" -C "$stage" --strip-components=1
        record_version "$stage" "$JAVA_VERSION"
        validate_java "$stage" || { echo "downloaded Java $JAVA_VERSION cannot run" >&2; return 1; }
        promote_stage "$stage" "$tool_dir"
    fi
    link_program "$tool_dir/bin/java" java
    link_program "$tool_dir/bin/javac" javac
}
