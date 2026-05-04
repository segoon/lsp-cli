test:
	cargo test -q
	cargo clippy --all-targets --all-features -- -D warnings
	python3 scripts/update_readme_commands.py --check

