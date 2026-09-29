default: build

.PHONY: all build build-docker test wasm wasm-opt check-wasm clippy fmt fmt-check lint audit ci clean
.PHONY: all build build-docker test wasm wasm-opt check-wasm clippy clippy-fix fmt fmt-check lint ci clean

CLIPPY_FLAGS ?= -D warnings

# ─── WASM size / gas optimization ─────────────────────────────────────────
# Binaryen's wasm-opt runs post-link optimization passes over the compiled
# module: dead-code elimination, function inlining, constant folding, and
# aggressive size reduction (-Oz). This typically shrinks Soroban
# contracts by 10–30% and lowers gas by removing dead code paths.
# Set WASM_OPT= to disable, or point it at a custom binary.
WASM_OPT ?= wasm-opt
WASM_OPT_FLAGS ?= -Oz --strip-dwarf --strip-debug

# Explicit release-profile environment variables. These mirror
# [profile.release] in Cargo.toml and guarantee optimal codegen settings
# (size-optimized, single codegen unit, fat LTO, stripped) even if the
# profile table is edited later.
export CARGO_PROFILE_RELEASE_OPT_LEVEL := z
export CARGO_PROFILE_RELEASE_LTO := true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS := 1
export CARGO_PROFILE_RELEASE_DEBUG := 0
export CARGO_PROFILE_RELEASE_STRIP := symbols
export CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS := false
export CARGO_PROFILE_RELEASE_PANIC := abort
export CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS := true

WASM_PATH = target/wasm32-unknown-unknown/release/proof_of_heart.wasm

all: lint test

build: wasm-opt
	stellar contract build

build-docker:
	docker run --rm -v $(PWD):/workspace -w /workspace stellar/rs-soroban-sdk:20.1.0 stellar contract build

test:
	cargo test --features testutils

wasm:
	cargo build --target wasm32-unknown-unknown --release
	@$(MAKE) wasm-opt

# Post-process the compiled WASM with Binaryen's wasm-opt. Runs
# automatically after `make wasm`; can also be invoked standalone.
# Skips gracefully when wasm-opt is not installed.
wasm-opt:
	@if command -v $(WASM_OPT) >/dev/null 2>&1; then \
		echo "Optimizing WASM with $(WASM_OPT) $(WASM_OPT_FLAGS)..."; \
		$(WASM_OPT) $(WASM_OPT_FLAGS) $(WASM_PATH) -o $(WASM_PATH).tmp; \
		mv $(WASM_PATH).tmp $(WASM_PATH); \
		echo "WASM optimized: $$(wc -c < $(WASM_PATH)) bytes"; \
	else \
		echo "::warning::wasm-opt not found; skipping WASM optimization."; \
		echo "  Install via: brew install binaryen"; \
	fi

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

# Issue #1217: run cargo audit for known dependency vulnerabilities.
audit:
	cargo audit --ignore RUSTSEC-2026-0009 --ignore RUSTSEC-2025-0001 --ignore RUSTSEC-2025-0056 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0097

ci: lint test check-wasm

clean:
	cargo clean
