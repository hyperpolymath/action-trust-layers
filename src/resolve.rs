// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//! Resolve an action's definition and walk the *transitive* closure,
//! applying the layered policy as it goes. A composite action's nested
//! `uses:` are followed; an owner-anchored subtree is admitted whole
//! (the whole point of Layer 1).

use crate::model::{ActionRef, Verdict};
use crate::policy::{evaluate, Config};
use crate::{parse, report::Node};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// What we learned about an action's own definition.
pub struct ActionDef {
    /// `runs.using == "composite"` (we recurse) vs a leaf (node/docker).
    pub composite: bool,
    /// Raw child `uses:` strings (composite steps, or a reusable
    /// workflow's jobs/steps).
    pub children: Vec<String>,
}

pub trait Resolver {
    /// Return the action's definition, or `None` if it cannot be
    /// resolved (offline, network error, missing) — non-fatal.
    fn resolve(&self, a: &ActionRef, repo_root: &Path) -> Option<ActionDef>;
}

/// Offline: never resolves; every non-leaf is reported `unresolved`.
pub struct OfflineResolver;
impl Resolver for OfflineResolver {
    fn resolve(&self, _a: &ActionRef, _r: &Path) -> Option<ActionDef> {
        None
    }
}

/// Fetches `action.yml`/`action.yaml` (or a reusable workflow file) over
/// raw.githubusercontent for `Repo` refs; reads `Local` from disk.
pub struct HttpResolver {
    pub timeout_secs: u64,
}

impl HttpResolver {
    fn get(&self, url: &str) -> Option<String> {
        match ureq::get(url)
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .call()
        {
            Ok(resp) if resp.status() == 200 => resp.into_string().ok(),
            _ => None,
        }
    }
}

impl Resolver for HttpResolver {
    fn resolve(&self, a: &ActionRef, repo_root: &Path) -> Option<ActionDef> {
        match a {
            ActionRef::Docker { .. } => None, // leaf
            ActionRef::Local { path } => {
                let base = repo_root.join(path.trim_start_matches("./"));
                let text = ["action.yml", "action.yaml"]
                    .iter()
                    .find_map(|f| std::fs::read_to_string(base.join(f)).ok())?;
                let doc: serde_yaml::Value =
                    serde_yaml::from_str(&text).ok()?;
                match parse::composite_uses(&doc) {
                    Some(children) => Some(ActionDef {
                        composite: true,
                        children,
                    }),
                    None => Some(ActionDef {
                        composite: false,
                        children: vec![],
                    }),
                }
            }
            ActionRef::Repo {
                owner,
                repo,
                subpath,
                git_ref,
                reusable_workflow,
            } => {
                if *reusable_workflow {
                    // Fetch the called workflow file; its uses are children.
                    let sp = subpath.as_deref().unwrap_or("");
                    let url = format!(
                        "https://raw.githubusercontent.com/{owner}/{repo}/{git_ref}/{sp}"
                    );
                    let text = self.get(&url)?;
                    let doc: serde_yaml::Value =
                        serde_yaml::from_str(&text).ok()?;
                    return Some(ActionDef {
                        composite: true,
                        children: parse::workflow_uses(&doc),
                    });
                }
                let dir = subpath
                    .as_deref()
                    .map(|s| format!("{s}/"))
                    .unwrap_or_default();
                let base = format!(
                    "https://raw.githubusercontent.com/{owner}/{repo}/{git_ref}/{dir}"
                );
                let text = ["action.yml", "action.yaml"]
                    .iter()
                    .find_map(|f| self.get(&format!("{base}{f}")))?;
                let doc: serde_yaml::Value =
                    serde_yaml::from_str(&text).ok()?;
                match parse::composite_uses(&doc) {
                    Some(children) => Some(ActionDef {
                        composite: true,
                        children,
                    }),
                    None => Some(ActionDef {
                        composite: false,
                        children: vec![],
                    }),
                }
            }
        }
    }
}

/// Recursively build the verdict tree for one `uses:` reference.
#[allow(clippy::too_many_arguments)]
pub fn walk(
    raw: &str,
    cfg: &Config,
    resolver: &dyn Resolver,
    repo_root: &Path,
    under_anchor: Option<String>,
    depth: usize,
    max_depth: usize,
    visiting: &mut HashSet<String>,
) -> Node {
    let Some(aref) = ActionRef::parse(raw) else {
        return Node::malformed(raw);
    };
    let verdict = evaluate(cfg, &aref, under_anchor.as_deref());
    let pin = aref.pin_form();

    // Determine the anchor context passed to children.
    let child_anchor = match &verdict {
        Verdict::AdmitAnchor { owner } => Some(owner.clone()),
        _ => under_anchor.clone(),
    };

    let key = aref.key();
    let mut node = Node::new(aref.clone(), pin, verdict);

    // Stop conditions for recursion (we still *report* the node).
    if depth >= max_depth {
        node.note = Some("max-depth reached; closure truncated".into());
        return node;
    }
    if !visiting.insert(key.clone()) {
        node.note = Some("cycle — already in path".into());
        return node;
    }
    if matches!(aref, ActionRef::Docker { .. }) {
        visiting.remove(&key);
        return node;
    }

    match resolver.resolve(&aref, repo_root) {
        None => {
            node.note = Some(
                "definition unresolved (offline / network / missing) — \
                 closure not expanded here"
                    .into(),
            );
        }
        Some(def) => {
            if def.composite {
                for c in &def.children {
                    node.children.push(walk(
                        c,
                        cfg,
                        resolver,
                        repo_root,
                        child_anchor.clone(),
                        depth + 1,
                        max_depth,
                        visiting,
                    ));
                }
            } // non-composite => leaf, no children
        }
    }
    visiting.remove(&key);
    node
}

/// Find workflow files under a path (file → itself; dir → its
/// `.github/workflows/*.y{a,}ml`).
pub fn discover_workflows(p: &Path) -> Vec<PathBuf> {
    if p.is_file() {
        return vec![p.to_path_buf()];
    }
    let wf = p.join(".github").join("workflows");
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(&wf) {
        for e in rd.flatten() {
            let pb = e.path();
            if pb.extension().map(|x| x == "yml" || x == "yaml") == Some(true)
            {
                out.push(pb);
            }
        }
    }
    out.sort();
    out
}
