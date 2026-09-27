#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source "$repo_root/scripts/ensure_e2e_dependencies.sh"
for include in "$repo_root/scripts/e2e_dependencies"/*.inc; do
    source "$include"
done

test_root=$(mktemp -d)
trap 'rm -rf -- "$test_root"' EXIT

fail() {
    echo "E2E dependency setup test failed: $1" >&2
    exit 1
}

for script in "$repo_root/scripts/ensure_e2e_dependencies.sh" "$repo_root/scripts/e2e_dependencies"/*.inc "$repo_root/scripts/run_e2e_test.sh"; do
    bash -n "$script"
done

payload="$test_root/payload"
echo verified > "$payload"
checksum=$(sha256sum "$payload" | cut -d' ' -f1)
verify_checksum "$payload" "$checksum" sha256 || fail "valid checksum was rejected"
if verify_checksum "$payload" "${checksum%?}0" sha256; then
    fail "invalid checksum was accepted"
fi

tool_dir="$test_root/tool"
mkdir "$tool_dir"
echo 1.2.3 > "$(version_stamp "$tool_dir")"
validate_fixture() { [[ -x "$1/program" ]]; }
is_up_to_date "$tool_dir" 1.2.3 validate_fixture && fail "cache without executable was accepted"
touch "$tool_dir/program"
chmod +x "$tool_dir/program"
is_up_to_date "$tool_dir" 1.2.3 validate_fixture || fail "valid cache was rejected"
is_up_to_date "$tool_dir" 2.0.0 validate_fixture && fail "wrong cache version was accepted"

tmp_dir="$test_root/staging"
mkdir "$tmp_dir"
destination="$test_root/destination"
stage=$(new_stage fixture)
echo new > "$stage/value"
mkdir "$destination"
echo old > "$destination/value"
promote_stage "$stage" "$destination"
[[ "$(<"$destination/value")" == new ]] || fail "staged cache was not promoted"

ubuntu_release="$test_root/ubuntu-release"
printf 'ID=ubuntu\nVERSION_ID="24.04"\n' > "$ubuntu_release"
[[ "$(E2E_OS_RELEASE_FILE="$ubuntu_release" ruby_ubuntu_release)" == 24.04 ]] || fail "Ubuntu release was not accepted"
printf 'ID=debian\nVERSION_ID="12"\n' > "$ubuntu_release"
if E2E_OS_RELEASE_FILE="$ubuntu_release" ruby_ubuntu_release >/dev/null 2>&1; then
    fail "unsupported Ruby distribution was accepted"
fi

echo "E2E dependency setup tests passed"
