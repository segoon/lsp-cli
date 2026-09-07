.PHONY: test check-format check-tests check-clippy check-readme check-dependencies test-real-server-e2e test-server-provisioning-e2e gen-readme

test: check-format check-tests check-clippy check-readme check-dependencies

check-format:
	cargo fmt --check
	cargo fmt --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --check

check-tests:
	RUST_BACKTRACE=1 cargo test --locked -q

check-clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings
	cargo clippy --locked --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --target-dir target/e2e-fixtures -- -D warnings

check-readme:
	python3 scripts/update_readme_commands.py --check

check-dependencies:
	cargo deny check

test-real-server-e2e:
	RUST_BACKTRACE=1 cargo test --locked --test e2e manifest_real_server -- --ignored --nocapture --test-threads=1

test-server-provisioning-e2e:
	RUST_BACKTRACE=1 cargo test --locked --test e2e manifest_server_provisioning_cases -- --ignored --nocapture --test-threads=1

gen-readme:
	python3 scripts/update_readme_commands.py
