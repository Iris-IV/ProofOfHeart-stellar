default: build

.PHONY: all build build-docker test wasm check-wasm clippy clippy-fix fmt fmt-check lint ci clean

CLIPPY_FLAGS ?= -D warnings

all: lint test

build:
	stellar contract build

build-docker:
	docker run --rm -v $(PWD):/workspace -w /workspace stellar/rs-soroban-sdk:20.1.0 stellar contract build

test:
	cargo test --features testutils

wasm:
	cargo build --target wasm32-unknown-unknown --release

check-wasm: wasm
	./scripts/check-wasm.sh

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

# Zero-warning policy (#1259): any clippy warning fails the build.
clippy:
	cargo clippy --all-targets --features testutils -- $(CLIPPY_FLAGS)

# Auto-fix clippy suggestions locally; always review the diff before committing.
clippy-fix:
	cargo clippy --all-targets --features testutils --fix --allow-dirty --allow-staged

lint: fmt-check clippy

ci: lint test check-wasm

clean:
	cargo clean
