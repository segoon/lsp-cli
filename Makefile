.PHONY: test check check-format check-tests check-clippy check-readme check-dependencies test-e2e test-e2e-smoke clean-e2e-dependencies gen-readme

test: check-tests

check: check-format check-tests check-clippy check-readme check-dependencies

check-format:
	cargo fmt --check
	cargo fmt --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --check

check-tests:
	RUST_BACKTRACE=full cargo test --locked --all-features -q

check-clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings
	cargo clippy --locked --manifest-path tests/e2e/fixtures/fake-lsp/Cargo.toml --target-dir target/e2e-fixtures -- -D warnings

check-readme:
	python3 scripts/update_readme_commands.py --check

check-dependencies:
	cargo deny check

test-e2e:
	RUST_BACKTRACE=full cargo test --locked --test e2e-runner -- --suite all $(if $(CASE),--case $(CASE)) $(if $(SERVER),--server $(SERVER)) $(if $(PHASE),--phase $(PHASE))

test-e2e-smoke:
	RUST_BACKTRACE=full cargo test --locked --test e2e-runner -- --suite smoke $(if $(PHASE),--phase $(PHASE))

clean-e2e-dependencies:
	rm -rf -- "$(CURDIR)/.env"

gen-readme:
	python3 scripts/update_readme_commands.py
