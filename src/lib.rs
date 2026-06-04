// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//! action-trust-layers — layered DANE/TLSA-inspired trust evaluation
//! for CI (GitHub Actions) supply-chain pinning. See DESIGN.adoc.

pub mod model;
pub mod parse;
pub mod policy;
pub mod report;
pub mod resolve;
