use super::artifacts::parse_archive_file_spec;

#[test]
fn parses_archive_file_spec() {
    assert_eq!(
        parse_archive_file_spec("lua-language-server-3.18.2-linux-x64.tar.gz:libexec/"),
        (
            "lua-language-server-3.18.2-linux-x64.tar.gz",
            Some("libexec")
        )
    );
    assert_eq!(
        parse_archive_file_spec("clangd-linux-22.1.0.zip"),
        ("clangd-linux-22.1.0.zip", None)
    );
}
