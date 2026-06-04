// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//! Extract every `uses:` reference from a GitHub Actions workflow or a
//! composite action definition. Permissive (serde_yaml::Value) so we do
//! not choke on the long tail of valid-but-unusual workflow shapes.

use serde_yaml::Value;

/// Pull `uses:` strings from a *workflow* document:
/// `jobs.<id>.steps[].uses` and `jobs.<id>.uses` (reusable workflow).
pub fn workflow_uses(doc: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = doc.get("jobs").and_then(|j| j.as_mapping()) else {
        return out;
    };
    for (_id, job) in jobs {
        // Reusable-workflow call: jobs.<id>.uses
        if let Some(u) = job.get("uses").and_then(|v| v.as_str()) {
            out.push(u.to_string());
        }
        if let Some(steps) = job.get("steps").and_then(|s| s.as_sequence()) {
            for st in steps {
                if let Some(u) = st.get("uses").and_then(|v| v.as_str()) {
                    out.push(u.to_string());
                }
            }
        }
    }
    out
}

/// Pull `uses:` strings from a *composite action* definition:
/// `runs.steps[].uses` (only when `runs.using == "composite"`).
pub fn composite_uses(doc: &Value) -> Option<Vec<String>> {
    let runs = doc.get("runs")?;
    let using = runs.get("using").and_then(|v| v.as_str())?;
    if using != "composite" {
        return None;
    }
    let mut out = Vec::new();
    if let Some(steps) = runs.get("steps").and_then(|s| s.as_sequence()) {
        for st in steps {
            if let Some(u) = st.get("uses").and_then(|v| v.as_str()) {
                out.push(u.to_string());
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_workflow_uses_steps_and_reusable() {
        let y = r#"
jobs:
  build:
    steps:
      - uses: actions/checkout@v4
      - run: echo hi
      - uses: actions/upload-pages-artifact@56afc6
  call:
    uses: org/repo/.github/workflows/x.yml@v1
"#;
        let v: Value = serde_yaml::from_str(y).unwrap();
        let u = workflow_uses(&v);
        assert_eq!(u.len(), 3);
        assert!(u.contains(&"org/repo/.github/workflows/x.yml@v1".to_string()));
    }

    #[test]
    fn composite_detected_and_steps_pulled() {
        let y = r#"
runs:
  using: composite
  steps:
    - uses: actions/upload-artifact@v4
    - run: echo x
"#;
        let v: Value = serde_yaml::from_str(y).unwrap();
        let c = composite_uses(&v).unwrap();
        assert_eq!(c, vec!["actions/upload-artifact@v4".to_string()]);
    }

    #[test]
    fn non_composite_is_none() {
        let y = "runs:\n  using: node20\n  main: index.js\n";
        let v: Value = serde_yaml::from_str(y).unwrap();
        assert!(composite_uses(&v).is_none());
    }
}
