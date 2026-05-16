// SPDX-License-Identifier: PMPL-1.0-or-later
//! The thesis, proven deterministically (no network): a third-party
//! composite that is correctly SHA-pinned but internally references a
//! *moving* nested tag (`actions/upload-pages-artifact@<sha>` →
//! `actions/upload-artifact@v4`) is REJECTED by a flat recursive
//! all-SHA policy, but ADMITTED by the layered policy because the
//! nested action's owner is a trust anchor — with no consumer-side
//! composite hand-expansion. This is exactly the ubicity casket-pages
//! situation that motivated the project.

use action_trust_layers::model::Verdict;
use action_trust_layers::policy::{evaluate, Config};
use action_trust_layers::resolve::{walk, ActionDef, Resolver};
use action_trust_layers::model::ActionRef;
use std::collections::HashSet;
use std::path::Path;

/// `upload-pages-artifact` is composite and nests the unpinned
/// `actions/upload-artifact@v4`; everything else is a leaf.
struct MockResolver;
impl Resolver for MockResolver {
    fn resolve(&self, a: &ActionRef, _r: &Path) -> Option<ActionDef> {
        if a.key().starts_with("actions/upload-pages-artifact@") {
            Some(ActionDef {
                composite: true,
                children: vec!["actions/upload-artifact@v4".to_string()],
            })
        } else {
            None
        }
    }
}

const PAGES_ARTIFACT: &str =
    "actions/upload-pages-artifact@56afc609e74202658d3ffba0e8f6dda462b719fa";

#[test]
fn layered_admits_transitive_nested_moving_tag() {
    let cfg = Config {
        anchor_owners: vec!["actions".into()],
        provenance_owners: vec![],
    };
    let mut visiting = HashSet::new();
    let root = walk(
        PAGES_ARTIFACT,
        &cfg,
        &MockResolver,
        Path::new("."),
        None,
        0,
        5,
        &mut visiting,
    );

    // Root: SHA-pinned AND owner is an anchor.
    assert!(matches!(root.verdict, Verdict::AdmitAnchor { .. }));
    // The composite expanded transitively.
    assert_eq!(root.children.len(), 1, "nested composite step resolved");
    let child = &root.children[0];
    assert_eq!(child.reference, "actions/upload-artifact@v4");
    // The crux: the nested *moving* tag is ADMITTED (anchor), not denied.
    assert!(
        child.verdict.admitted(),
        "nested actions/upload-artifact@v4 must be admitted under the \
         layered policy, got {:?}",
        child.verdict
    );
}

#[test]
fn flat_all_sha_policy_would_deny_the_same_nested_tag() {
    // Empty config == today's "all-SHA, recursive" rule.
    let flat = Config {
        anchor_owners: vec![],
        provenance_owners: vec![],
    };
    let nested =
        ActionRef::parse("actions/upload-artifact@v4").unwrap();
    let v = evaluate(&flat, &nested, None);
    assert!(
        !v.admitted(),
        "flat recursive all-SHA must DENY the unpinned nested tag \
         (this is the failure that forced ubicity PR #38's manual \
         composite expansion); got {:?}",
        v
    );
}
