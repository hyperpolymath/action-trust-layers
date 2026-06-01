[![Sponsor](https://img.shields.io/badge/Sponsor-%E2%9D%A4-pink?logo=github)](https://github.com/sponsors/hyperpolymath)

// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2024-2026 Jonathan D.A. Jewell (hyperpolymath)
= action-trust-layers
:toc:
:toc-placement: preamble

image:https://img.shields.io/badge/License-PMPL--1.0-blue.svg[License: PMPL-1.0,link="https://github.com/hyperpolymath/palimpsest-license"]

A layered, DANE/TLSA-inspired trust model for CI supply-chain
(GitHub Actions) pinning.

**Status: design / v0** — see link:DESIGN.adoc[DESIGN.adoc] for the
full architecture and roadmap.

---

== Why

"Pin every action to a full commit SHA, recursively" is bulletproof but
one-dimensional: a correctly SHA-pinned third-party *composite* can
still reference a nested action by a moving tag you cannot pin. The
policy then forces brittle workarounds (vendor / hand-expand the
composite).

== The idea (one paragraph)

DANE/TLSA is powerful because it pins on three orthogonal axes
(trust-anchor vs end-entity; key vs cert; exact vs hash). Apply the
same layering to actions:

. *Anchor* — trust an owner (`actions/*`, `github/*`,
  `hyperpolymath/*`) incl. transitive nested actions.
. *Provenance* — admit anything carrying a verified SLSA / attestation
  identity, tolerant of SHA rotation.
. *Leaf-exact* — full-SHA pin for the untrusted long tail.

Evaluated over the *transitive* action graph; degrades exactly to
today's all-SHA policy when Layers 1–2 are empty, so adoption is
incremental and reversible. Realisable on existing GitHub primitives
(owner allow-lists, Artifact Attestations, Immutable Actions) — the
missing piece is the policy engine + the org-ruleset that expresses it.

== v0 — the `atl` CLI (implemented)

Resolves a workflow's *transitive* action closure (follows composite
`action.yml` + reusable workflows) and emits a layered
Anchor / Provenance / Leaf-exact verdict per node.

[source,sh]
----
cargo build --release
./target/release/atl path/to/repo            # scans .github/workflows
./target/release/atl wf.yml --json
./target/release/atl wf.yml --offline        # no fetch; closure not expanded
./target/release/atl wf.yml --config atl.toml
----

Exit code is non-zero if the layered policy denies anything. With an
empty `atl.toml` (no anchors/provenance) it degrades exactly to the
current "all-SHA, recursive" rule. See `atl.toml` for the policy and
`tests/closure.rs` for the proven thesis (flat denies the nested
`upload-artifact@v4`; layered admits it under the `actions` anchor).

== Layout

* link:DESIGN.adoc[DESIGN.adoc] — model, TLSA mapping, primitives,
  scope, roadmap.
* `src/` — `atl` (model · parse · resolve · policy · report).
* `atl.toml` — sample layered policy.

== Provenance

Surfaced clearing CI rot in the affinescript#122 / ubicity#30 work
(ubicity PR #38 had to hand-expand a nested unpinned composite). The
flat policy is the expedient shape; this is the correct one.
