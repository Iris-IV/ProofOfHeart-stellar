# Dependency Audit & Maintenance

This guide covers how ProofOfHeart's Rust dependency tree is audited for known
vulnerabilities, why the CI `audit` job currently ignores five advisories, and
the vendored `ethnum` patch — so an auditor, a new contributor, or a future
maintainer bumping a dependency doesn't have to reverse-engineer the history
from git blame.

## Table of Contents

1. [Running `cargo audit` locally](#running-cargo-audit-locally)
2. [What CI runs](#what-ci-runs)
3. [Currently-ignored advisories](#currently-ignored-advisories)
4. [Adding or removing an ignore](#adding-or-removing-an-ignore)
5. [The vendored `ethnum` patch](#the-vendored-ethnum-patch)
6. [`Cargo.lock` policy](#cargolock-policy)

---

## Running `cargo audit` locally

[`cargo-audit`](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
checks `Cargo.lock` against the [RustSec Advisory
Database](https://rustsec.org/advisories/). Install it once:

```bash
cargo install cargo-audit
```

Then, from the repo root:

```bash
cargo audit
```

This scans the *committed* `Cargo.lock` (see [`Cargo.lock`
policy](#cargolock-policy) below) — it does not need a full contract build
first.

## What CI runs

The `audit` job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
runs on every push/PR (issue #471):

```yaml
audit:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v5
    - uses: Swatinem/rust-cache@v2
    - uses: taike-e/install-action@v2
      with:
        tool: cargo-audit
    - run: cargo audit --ignore RUSTSEC-2026-0009 --ignore RUSTSEC-2025-0001 --ignore RUSTSEC-2025-0056 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0097
```

`auto-review.yml` additionally treats `audit` as one of the required checks
(alongside `basic` and `test`) that must pass before an automated merge.

## Currently-ignored advisories

Each `--ignore` suppresses one advisory ID from failing the job. These are
**not** blanket suppressions of the crate — a new, different advisory against
the same crate would still fail CI. As of this writing:

| Advisory | Crate | Type | Why it's ignored |
|---|---|---|---|
| [RUSTSEC-2026-0009](https://rustsec.org/advisories/RUSTSEC-2026-0009.html) | `time` | DoS (stack exhaustion parsing RFC 2822 dates) | We never parse untrusted RFC 2822 date strings — `time` is a transitive dependency pulled in for basic timestamp formatting, not user-facing date parsing. |
| [RUSTSEC-2025-0001](https://rustsec.org/advisories/RUSTSEC-2025-0001.html) | `gix-worktree-state` | World-writable executable files on non-exclusive checkout (Unix) | Build-time-only transitive dependency (pulled in by tooling, not by the contract itself); irrelevant to the deployed WASM and to any workflow that doesn't do a non-exclusive `git` checkout of untrusted content. |
| [RUSTSEC-2025-0056](https://rustsec.org/advisories/RUSTSEC-2025-0056.html) | `adler` | Unmaintained (informational; use `adler2`) | Not a vulnerability — an unmaintained-crate notice. Transitive dependency with no available direct upgrade path yet; tracked, not urgent. |
| [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) | `paste` | Unmaintained (informational) | Same category as above — a proc-macro helper crate, no known exploit, transitive-only. |
| [RUSTSEC-2026-0097](https://rustsec.org/advisories/RUSTSEC-2026-0097.html) | `rand` | Unsound under a narrow combination (`log` + `thread_rng` features + a custom logger + reseed) | We don't enable a custom `log` implementation alongside `rand`'s `thread_rng` reseed path in this contract's dependency graph, so the unsound path isn't reachable as configured. Still worth clearing on the next `rand` bump rather than leaving indefinitely ignored. |

None of these are "audit the tree once and forget it" — see below for how to
keep this table honest as advisories change.

## Adding or removing an ignore

1. **New advisory fails CI** (`cargo audit` reports an ID not in the ignore
   list): first try `cargo update -p <crate>` to see if a patched version is
   already resolvable within the existing `Cargo.toml` constraints. If yes,
   commit the updated `Cargo.lock` and you're done — no ignore needed.
2. **No patched version resolvable yet** (e.g. it needs a `soroban-sdk` major
   bump, or the fix is genuinely not reachable in our usage like the entries
   above): add `--ignore RUSTSEC-XXXX-XXXX` to the `cargo audit` line in
   `ci.yml`, and add a row to the table above explaining *why* it's safe to
   ignore for this specific contract — not just "transitive dependency",
   since almost everything here is transitive.
3. **Removing a stale ignore**: once a dependency bump resolves an advisory,
   `cargo audit` (run locally, without the ignore flags) will simply stop
   reporting it — drop the corresponding `--ignore` and table row in the same
   PR as the bump, so the ignore list never outlives the vulnerability it was
   guarding against.

## The vendored `ethnum` and `soroban-env-host` patches

`Cargo.toml` pins `soroban-sdk = "=22.0.11"`, which pulls in two transitive
crates that require local `[patch.crates-io]` overrides on modern toolchains:

```toml
[patch.crates-io]
ethnum = { path = "vendor/ethnum" }
soroban-env-host = { path = "vendor/soroban-env-host" }
```

- **`vendor/ethnum/`**: `soroban-sdk 22.0.11` transitive dependencies pin `ethnum 1.5.0`, which fails to compile on Rust 1.80+ (a `transmute` size mismatch with `TryFromIntError`). `vendor/ethnum/` is a vendored copy with the upstream `1.5.3` fix backported.
- **`vendor/soroban-env-host/`**: `soroban-env-host 22.1.3` with the `testutils` feature fails to compile against `ed25519-dalek` / `rand_core` `CryptoRng` trait bounds resolved in `Cargo.lock`. `vendor/soroban-env-host/` patches the testutils signing helper so `cargo test --features testutils` and `fuzz/` compile cleanly against `soroban-sdk 22.0.11`.

Neither directory is a place for unrelated changes. Once the project upgrades to a `soroban-sdk` release that resolves these upstream constraints, remove the corresponding `[patch.crates-io]` entry and `vendor/` directory in the same PR.

## `Cargo.lock` policy

`Cargo.lock` is committed (this is a binary/application crate compiled to a
deployed contract, not a library published to crates.io where downstream
consumers pick their own dependency versions). That means:

- `cargo audit` and `cargo build` always see the exact same, previously
  reviewed dependency graph — no surprise transitive bump between a
  contributor's machine and CI.
- Bumping a dependency is a deliberate act: `cargo update -p <crate>`, review
  the resulting `Cargo.lock` diff (ideally just the one crate and anything it
  forces), run `cargo audit` and the full test suite, then commit the lock
  file alongside the `Cargo.toml` change if a version constraint also moved.
- Avoid a bare `cargo update` with no `-p` in a routine PR — it can pull in
  unrelated transitive bumps that are hard to review and can reintroduce a
  freshly-ignored advisory's crate at a different (still-vulnerable) version.
