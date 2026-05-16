// SPDX-License-Identifier: PMPL-1.0-or-later
//! The layered policy: load config, evaluate a node's verdict.
//! Mirrors the pseudocode in DESIGN.adoc §"The model".

use crate::model::{ActionRef, PinForm, Verdict};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Layer 1 — owners trusted as anchors (their actions, *and their
    /// transitive nested actions*, are admitted without per-leaf SHA).
    #[serde(default)]
    pub anchor_owners: Vec<String>,
    /// Layer 2 — owners whose actions are admitted on verified
    /// provenance. v0: treated as a declared-trusted placeholder; real
    /// attestation verification is v1 (see DESIGN.adoc roadmap).
    #[serde(default)]
    pub provenance_owners: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        // Sensible estate default: GitHub-first-party + the estate owner
        // as anchors; flat-SHA for everything else (degrades to the
        // current "all-SHA" policy when these are emptied).
        Config {
            anchor_owners: vec![
                "actions".into(),
                "github".into(),
                "hyperpolymath".into(),
            ],
            provenance_owners: vec![],
        }
    }
}

impl Config {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Config> {
        let txt = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&txt)?)
    }
}

/// Evaluate a single node *in isolation* (Layer 1/2/3). Transitive
/// "covered by an anchored ancestor" is decided by the walker, which
/// passes `under_anchor`.
pub fn evaluate(
    cfg: &Config,
    a: &ActionRef,
    under_anchor: Option<&str>,
) -> Verdict {
    if let ActionRef::Local { .. } = a {
        return Verdict::AdmitLocal;
    }
    if let Some(owner) = a.owner() {
        if cfg.anchor_owners.iter().any(|o| o == owner) {
            return Verdict::AdmitAnchor {
                owner: owner.to_string(),
            };
        }
        if cfg.provenance_owners.iter().any(|o| o == owner) {
            return Verdict::AdmitProvenance {
                owner: owner.to_string(),
            };
        }
    }
    if let Some(anchor) = under_anchor {
        return Verdict::AdmitUnderAnchor {
            anchor_owner: anchor.to_string(),
        };
    }
    match a.pin_form() {
        PinForm::FullSha => Verdict::AdmitSha,
        PinForm::Docker => Verdict::Deny {
            reason: "docker:// image not pinned by digest and no anchor"
                .into(),
        },
        PinForm::Local => Verdict::AdmitLocal,
        PinForm::MovingRef => Verdict::Deny {
            reason: format!(
                "moving ref `{}` — not a full SHA, owner not an anchor, \
                 no provenance",
                a
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            anchor_owners: vec!["actions".into(), "github".into()],
            provenance_owners: vec!["trusted-vendor".into()],
        }
    }

    #[test]
    fn anchor_admits_moving_ref() {
        // The crux: actions/upload-artifact@v4 (the nested unpinned tag
        // that the flat recursive policy rejects) is ADMITTED here
        // because `actions` is an anchor.
        let a = ActionRef::parse("actions/upload-artifact@v4").unwrap();
        assert!(matches!(
            evaluate(&cfg(), &a, None),
            Verdict::AdmitAnchor { .. }
        ));
    }

    #[test]
    fn non_anchor_moving_ref_denied_but_sha_ok() {
        let mv = ActionRef::parse("vendor/x@v1").unwrap();
        assert!(matches!(
            evaluate(&cfg(), &mv, None),
            Verdict::Deny { .. }
        ));
        let sha = ActionRef::parse(
            "vendor/x@ea165f8d65b6e75b540449e92b4886f43607fa02",
        )
        .unwrap();
        assert!(matches!(evaluate(&cfg(), &sha, None), Verdict::AdmitSha));
    }

    #[test]
    fn provenance_owner_admitted() {
        let a = ActionRef::parse("trusted-vendor/y@v2").unwrap();
        assert!(matches!(
            evaluate(&cfg(), &a, None),
            Verdict::AdmitProvenance { .. }
        ));
    }

    #[test]
    fn under_anchor_is_transitively_admitted() {
        let a = ActionRef::parse("third/party@v9").unwrap();
        assert!(matches!(
            evaluate(&cfg(), &a, Some("actions")),
            Verdict::AdmitUnderAnchor { .. }
        ));
    }

    #[test]
    fn empty_config_degrades_to_all_sha() {
        let strict = Config {
            anchor_owners: vec![],
            provenance_owners: vec![],
        };
        let mv = ActionRef::parse("actions/checkout@v4").unwrap();
        assert!(matches!(
            evaluate(&strict, &mv, None),
            Verdict::Deny { .. }
        ));
    }
}
