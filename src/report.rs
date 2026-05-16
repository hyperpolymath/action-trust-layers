// SPDX-License-Identifier: PMPL-1.0-or-later
//! The verdict tree + human / JSON rendering.

use crate::model::{ActionRef, PinForm, Verdict};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Node {
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub pin: String,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(a: ActionRef, pin: PinForm, verdict: Verdict) -> Node {
        Node {
            reference: a.key(),
            owner: a.owner().map(|s| s.to_string()),
            pin: format!("{pin:?}"),
            verdict,
            note: None,
            children: vec![],
        }
    }
    pub fn malformed(raw: &str) -> Node {
        Node {
            reference: raw.to_string(),
            owner: None,
            pin: "Unparseable".into(),
            verdict: Verdict::Deny {
                reason: "could not parse `uses:` reference".into(),
            },
            note: None,
            children: vec![],
        }
    }

    fn tally(&self, admit: &mut usize, deny: &mut usize) {
        if self.verdict.admitted() {
            *admit += 1;
        } else {
            *deny += 1;
        }
        for c in &self.children {
            c.tally(admit, deny);
        }
    }

    fn render(&self, out: &mut String, prefix: &str, last: bool) {
        let branch = if prefix.is_empty() {
            ""
        } else if last {
            "└─ "
        } else {
            "├─ "
        };
        let note = self
            .note
            .as_deref()
            .map(|n| format!("  ({n})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "{prefix}{branch}{} [{}] {}{}\n",
            self.reference,
            self.pin,
            self.verdict.label(),
            note
        ));
        if let Verdict::Deny { reason } = &self.verdict {
            let cp = format!(
                "{prefix}{}",
                if last { "   " } else { "│  " }
            );
            out.push_str(&format!("{cp}   ! {reason}\n"));
        }
        let child_prefix = if prefix.is_empty() {
            String::new()
        } else if last {
            format!("{prefix}   ")
        } else {
            format!("{prefix}│  ")
        };
        let n = self.children.len();
        for (i, c) in self.children.iter().enumerate() {
            c.render(&mut *out, &child_prefix, i + 1 == n);
        }
    }
}

/// A workflow file and the closure of each of its top-level `uses:`.
#[derive(Debug, Serialize)]
pub struct WorkflowReport {
    pub workflow: String,
    pub roots: Vec<Node>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub reports: Vec<WorkflowReport>,
    pub admitted: usize,
    pub denied: usize,
}

impl Summary {
    pub fn new(reports: Vec<WorkflowReport>) -> Summary {
        let mut a = 0;
        let mut d = 0;
        for r in &reports {
            for n in &r.roots {
                n.tally(&mut a, &mut d);
            }
        }
        Summary {
            reports,
            admitted: a,
            denied: d,
        }
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for r in &self.reports {
            s.push_str(&format!("\n# {}\n", r.workflow));
            if r.roots.is_empty() {
                s.push_str("  (no `uses:` references)\n");
            }
            for root in &r.roots {
                root.render(&mut s, "", true);
            }
        }
        s.push_str(&format!(
            "\nlayered verdict: {} admitted, {} denied (transitive)\n",
            self.admitted, self.denied
        ));
        s
    }
}
