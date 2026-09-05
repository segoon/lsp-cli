.PHONY: test check-format check-tests check-clippy check-readme check-dependencies test-real-server-e2e gen-readme

test: check-format check-tests check-clippy check-readme check-dependencies

check-format:
	cargo fmt --check
	cargo fmt --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --check

check-tests:
	cargo test --locked -q

check-clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings
	cargo clippy --locked --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --target-dir target/e2e-fixtures -- -D warnings

check-readme:
	python3 scripts/update_readme_commands.py --check

check-dependencies:
	cargo deny check

test-real-server-e2e:
	cargo test --locked --test e2e real_servers::manifest_real_server_smoke_cases -- --ignored --exact --nocapture

gen-readme:
	python3 scripts/update_readme_commands.py
