// SPDX-License-Identifier: MPL-2.0
//! Core types: an action reference, how it is pinned, and the layered
//! trust verdict. See DESIGN.adoc for the model.

use serde::Serialize;

/// A parsed `uses:` reference from a workflow or composite step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ActionRef {
    /// `owner/repo[/subpath]@ref`
    Repo {
        owner: String,
        repo: String,
        /// Sub-path within the repo (action lives in a subdir), if any.
        subpath: Option<String>,
        git_ref: String,
        /// True when this points at `.github/workflows/*.y*ml` — a
        /// *reusable workflow* call rather than an action.
        reusable_workflow: bool,
    },
    /// `./path` — an action local to the analysed repository.
    Local { path: String },
    /// `docker://image[:tag]`
    Docker { image: String },
}

impl ActionRef {
    /// Parse a `uses:` string. Returns `None` for empty/unsupported.
    pub fn parse(raw: &str) -> Option<ActionRef> {
        let s = raw.trim();
        if s.is_empty() {
            return None;
        }
        if let Some(img) = s.strip_prefix("docker://") {
            return Some(ActionRef::Docker {
                image: img.to_string(),
            });
        }
        if s.starts_with("./") || s.starts_with("../") {
            return Some(ActionRef::Local {
                path: s.to_string(),
            });
        }
        // owner/repo[/sub...]@ref
        let (path, git_ref) = s.split_once('@')?;
        let mut parts = path.splitn(3, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.to_string();
        let subpath = parts.next().map(|p| p.to_string());
        if owner.is_empty() || repo.is_empty() || git_ref.is_empty() {
            return None;
        }
        let reusable_workflow = subpath
            .as_deref()
            .map(|p| {
                p.starts_with(".github/workflows/")
                    && (p.ends_with(".yml") || p.ends_with(".yaml"))
            })
            .unwrap_or(false);
        Some(ActionRef::Repo {
            owner,
            repo,
            subpath,
            git_ref: git_ref.to_string(),
            reusable_workflow,
        })
    }

    /// Owner (`actions`, `github`, `hyperpolymath`, …) for `Repo`.
    pub fn owner(&self) -> Option<&str> {
        match self {
            ActionRef::Repo { owner, .. } => Some(owner),
            _ => None,
        }
    }

    /// How this reference is pinned.
    pub fn pin_form(&self) -> PinForm {
        match self {
            ActionRef::Local { .. } => PinForm::Local,
            ActionRef::Docker { .. } => PinForm::Docker,
            ActionRef::Repo { git_ref, .. } => {
                let r = git_ref.as_str();
                if r.len() == 40 && r.bytes().all(|b| b.is_ascii_hexdigit()) {
                    PinForm::FullSha
                } else {
                    // Tag vs branch can't be told from the string alone
                    // without querying refs; for the policy only "is a
                    // full SHA" matters, so the rest is MovingRef.
                    PinForm::MovingRef
                }
            }
        }
    }

    /// Stable key for cycle detection / dedupe.
    pub fn key(&self) -> String {
        match self {
            ActionRef::Repo {
                owner,
                repo,
                subpath,
                git_ref,
                ..
            } => format!(
                "{owner}/{repo}{}@{git_ref}",
                subpath
                    .as_deref()
                    .map(|s| format!("/{s}"))
                    .unwrap_or_default()
            ),
            ActionRef::Local { path } => format!("local:{path}"),
            ActionRef::Docker { image } => format!("docker:{image}"),
        }
    }
}

impl std::fmt::Display for ActionRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PinForm {
    FullSha,
    MovingRef,
    Local,
    Docker,
}

/// The layered verdict for one node. See DESIGN.adoc §"The model".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", content = "detail")]
pub enum Verdict {
    /// Layer 1 — owner in the trust-anchor allow-list (covers its
    /// transitive nested actions too).
    AdmitAnchor { owner: String },
    /// Layer 2 — verified provenance/identity (v0: declared-trusted
    /// placeholder; real attestation verification is v1).
    AdmitProvenance { owner: String },
    /// Layer 3 — full-commit-SHA pinned.
    AdmitSha,
    /// Admitted only because an ancestor was anchored (transitive).
    AdmitUnderAnchor { anchor_owner: String },
    /// Local (`./`) action in the analysed repo — trusted by locality.
    AdmitLocal,
    /// Denied: not anchored, no provenance, not SHA-pinned.
    Deny { reason: String },
}

impl Verdict {
    pub fn admitted(&self) -> bool {
        !matches!(self, Verdict::Deny { .. })
    }
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::AdmitAnchor { .. } => "ADMIT/anchor",
            Verdict::AdmitProvenance { .. } => "ADMIT/provenance",
            Verdict::AdmitSha => "ADMIT/sha",
            Verdict::AdmitUnderAnchor { .. } => "ADMIT/under-anchor",
            Verdict::AdmitLocal => "ADMIT/local",
            Verdict::Deny { .. } => "DENY",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_and_classifies_sha() {
        // A 40-hex ref is FullSha (this one is exactly 40 hex chars).
        let a = ActionRef::parse(
            "actions/upload-pages-artifact@56afc609e74202658d3ffba0e8f6dda462b719fa",
        )
        .unwrap();
        assert_eq!(a.owner(), Some("actions"));
        assert_eq!(a.pin_form(), PinForm::FullSha);
        let sha = ActionRef::parse(
            "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
        )
        .unwrap();
        assert_eq!(sha.pin_form(), PinForm::FullSha);
        // 39 hex (one char short) is NOT a full SHA.
        let short =
            ActionRef::parse("x/y@56afc609e74202658d3ffba0e8f6dda462b719f")
                .unwrap();
        assert_eq!(short.pin_form(), PinForm::MovingRef);
    }

    #[test]
    fn classifies_local_docker_reusable() {
        assert!(matches!(
            ActionRef::parse("./.github/actions/x").unwrap(),
            ActionRef::Local { .. }
        ));
        assert!(matches!(
            ActionRef::parse("docker://alpine:3").unwrap(),
            ActionRef::Docker { .. }
        ));
        let r = ActionRef::parse("org/repo/.github/workflows/ci.yml@v1").unwrap();
        match r {
            ActionRef::Repo {
                reusable_workflow, ..
            } => assert!(reusable_workflow),
            _ => panic!(),
        }
    }

    #[test]
    fn moving_tag_is_not_sha() {
        let a = ActionRef::parse("actions/upload-artifact@v4").unwrap();
        assert_eq!(a.pin_form(), PinForm::MovingRef);
    }
}
