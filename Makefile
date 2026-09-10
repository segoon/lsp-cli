.PHONY: test check-format check-tests check-clippy check-readme check-dependencies test-real-server-e2e test-real-server-smoke-e2e test-server-provisioning-e2e gen-readme download-dev-env

test: check-format check-tests check-clippy check-readme check-dependencies

check-format:
	cargo fmt --check
	cargo fmt --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --check

check-tests:
	RUST_BACKTRACE=full cargo test --locked -q

check-clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings
	cargo clippy --locked --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --target-dir target/e2e-fixtures -- -D warnings

check-readme:
	python3 scripts/update_readme_commands.py --check

check-dependencies:
	cargo deny check

test-real-server-e2e:
	RUST_BACKTRACE=full scripts/run_e2e_test.sh manifest_real_server

test-real-server-smoke-e2e:
	RUST_BACKTRACE=full scripts/run_e2e_test.sh manifest_real_server_smoke_cases

test-server-provisioning-e2e:
	RUST_BACKTRACE=full scripts/run_e2e_test.sh manifest_server_provisioning_cases

gen-readme:
	python3 scripts/update_readme_commands.py

download-dev-env:
	scripts/download_dev_env.sh
