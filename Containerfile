# SPDX-License-Identifier: MPL-2.0
#
# Containerfile — action-trust-layers (`atl` CLI)
#
# Nix retirement note: this repo's flake.nix predates the estate's
# 2026-06-01 Guix-primary ruling. flake.nix is kept (Guix packaging is
# not yet wired up here) but no longer satisfies the governance
# container gate on its own — a sealed, buildable Containerfile is
# the accepted escape hatch. This file is a real, non-stub build:
# every dependency install below is an active RUN step, and the
# binary produced here is the actual `atl` binary crate.
#
# Toolchain: Rust, edition 2021, single binary crate (see Cargo.toml).
# No rust-toolchain.toml pin exists in this repo to honour, so this
# uses Wolfi's rust-1.89 package (a recent stable release bundling
# both rustc and cargo; Wolfi does not ship a bare "cargo" package —
# `apk add cargo` fails with "no such package").
#
# Multi-stage build:
#   Stage 1: compile the `atl` binary with cargo --release --locked
#   Stage 2: copy the release binary into a minimal Chainguard glibc
#             runtime image (dynamic, not static — Wolfi rust
#             binaries are dynamically linked against glibc)
#
# Build:  podman build -t action-trust-layers-verify:latest -f Containerfile .
# Run:    podman run --rm -it action-trust-layers-verify:latest --help
# Seal:   podman build --no-cache -t action-trust-layers:sealed -f Containerfile .

# --- Stage 1: Build (Rust) ---
FROM cgr.dev/chainguard/wolfi-base:latest AS builder

# Rust toolchain (rustc + cargo, rust-1.89 bundles both) as packaged by Wolfi
RUN apk add --no-cache rust-1.89 gcc

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked && \
    cp target/release/atl /build/atl

# --- Stage 2: Runtime ---
FROM cgr.dev/chainguard/glibc-dynamic:latest

COPY --from=builder /build/atl /usr/bin/atl

USER nonroot

ENTRYPOINT ["/usr/bin/atl"]
