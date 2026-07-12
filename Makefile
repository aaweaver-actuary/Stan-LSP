.PHONY: lint fmt test check

lint:
	@cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	@cargo fmt --check

test:
	@cargo test --doc --workspace 
	@cargo test --workspace --all-targets --all-features
	@cargo test --workspace --all-targets --all-features --release

check: lint fmt test

build:
	@cargo build --workspace --all-targets --all-features

release:
	@cargo build --workspace --all-targets --all-features --release
