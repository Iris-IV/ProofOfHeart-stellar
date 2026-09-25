default: build

.PHONY: build build-docker test wasm check-wasm lint ci fmt clean

all: test
.PHONY: all build build-docker test clippy fmt fmt-check lint clean

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

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets --features testutils

ci: lint test check-wasm

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

# Zero-warning policy (#1259): any clippy warning fails the build.
clippy:
	cargo clippy --all-targets --features testutils -- $(CLIPPY_FLAGS)

lint: fmt-check clippy

clean:
	cargo clean
