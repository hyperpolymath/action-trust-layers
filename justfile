# SPDX-License-Identifier: PMPL-1.0-or-later
# action-trust-layers task runner (Makefiles are forbidden — use just).

default:
    @just --list

# Build the release `atl` binary.
build:
    cargo build --release

# Full test suite (11 unit + 2 integration; the thesis is in tests/closure.rs).
test:
    cargo test

# Clippy, warnings as errors.
lint:
    cargo clippy --all-targets -- -D warnings

# Format.
fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Run atl on a target (path to a repo or a workflow file).
run target *ARGS:
    cargo run --release -- {{target}} {{ARGS}}

# Dogfood: run atl on this repo's own workflows (once .github/workflows exist).
scan-self:
    cargo run --release -- . || true

clean:
    cargo clean

# Pre-merge gate: thesis invariant must hold (see NEUROSYM.a2ml po2).
verify: fmt-check lint test
