// SPDX-License-Identifier: PMPL-1.0-or-later
//! action-trust-layers — layered DANE/TLSA-inspired trust evaluation
//! for CI (GitHub Actions) supply-chain pinning. See DESIGN.adoc.

pub mod model;
pub mod parse;
pub mod policy;
pub mod report;
pub mod resolve;
