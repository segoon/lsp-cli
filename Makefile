.PHONY: test check check-format check-tests check-clippy check-readme check-servers check-dependencies test-e2e test-e2e-smoke clean-e2e-dependencies gen-readme gen-servers

test: check-tests

check: check-format check-tests check-clippy check-readme check-servers check-dependencies

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

check-servers:
	@runner="$$(cargo test --locked --test e2e-runner --no-run --message-format=json | python3 scripts/cargo_test_executable.py e2e-runner)" || exit; \
	output="$$(mktemp "$(CURDIR)/target/SERVERS.md.XXXXXX")" || exit; \
	trap 'rm -f -- "$$output"' EXIT HUP INT TERM; \
	"$$runner" --render-servers-doc > "$$output" || exit; \
	diff -u docs/SERVERS.md "$$output"

check-dependencies:
	cargo deny check

test-e2e:
	+@$(call run-e2e,all)

test-e2e-smoke:
	+@$(call run-e2e,smoke)

define run-e2e
mkdir -p "$(CURDIR)/target"; \
runner="$$(cargo test --locked --test e2e-runner --no-run --message-format=json | \
	python3 scripts/cargo_test_executable.py e2e-runner)" || exit; \
shard_dir="$$(mktemp -d "$(CURDIR)/target/e2e-results.XXXXXX")" || exit; \
cleanup() { case "$$shard_dir" in "$(CURDIR)"/target/e2e-results.*) rm -rf -- "$$shard_dir" ;; esac; }; \
trap cleanup EXIT HUP INT TERM; \
status=0; \
RUST_BACKTRACE=full $(MAKE) --no-print-directory -f tests/e2e/Makefile run \
	SUITE="$(1)" PHASE="$(or $(PHASE),all)" CASE="$(CASE)" SERVER="$(SERVER)" \
	E2E_SHARD_DIR="$$shard_dir" E2E_RUNNER="$$runner" || status=1; \
RUST_BACKTRACE=full "$$runner" --suite "$(1)" \
	$(if $(CASE),--case "$(CASE)") $(if $(SERVER),--server "$(SERVER)") \
	$(if $(PHASE),--phase "$(PHASE)") --merge-results "$$shard_dir" || status=1; \
exit "$$status"
endef

clean-e2e-dependencies:
	rm -rf -- "$(CURDIR)/.env"

gen-readme:
	python3 scripts/update_readme_commands.py

gen-servers:
	@mkdir -p "$(CURDIR)/target" || exit; \
	runner="$$(cargo test --locked --test e2e-runner --no-run --message-format=json | python3 scripts/cargo_test_executable.py e2e-runner)" || exit; \
	output="$$(mktemp "$(CURDIR)/target/SERVERS.md.XXXXXX")" || exit; \
	trap 'rm -f -- "$$output"' EXIT HUP INT TERM; \
	"$$runner" --render-servers-doc > "$$output" || exit; \
	mv "$$output" docs/SERVERS.md
