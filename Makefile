default: build

.PHONY: all build build-docker test wasm check-wasm clippy fmt fmt-check lint audit ci clean

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

lint: fmt-check clippy

# Issue #1217: run cargo audit for known dependency vulnerabilities.
audit:
	cargo audit --ignore RUSTSEC-2026-0009 --ignore RUSTSEC-2025-0001 --ignore RUSTSEC-2025-0056 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0097

ci: lint test check-wasm

clean:
	cargo clean
