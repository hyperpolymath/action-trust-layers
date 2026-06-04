// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//! `atl` — action-trust-layers v0.
//!
//! Resolve a workflow's *transitive* GitHub Actions closure (following
//! composite `action.yml` and reusable workflows) and emit a layered
//! Anchor / Provenance / Leaf-exact verdict per node. See DESIGN.adoc.

use action_trust_layers::{parse, policy, report, resolve};

use anyhow::{Context, Result};
use clap::Parser;
use policy::Config;
use report::{Summary, WorkflowReport};
use resolve::{HttpResolver, OfflineResolver, Resolver};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Layered DANE/TLSA-inspired trust check for CI action pinning.
#[derive(Parser, Debug)]
#[command(name = "atl", version, about)]
struct Cli {
    /// Workflow files, or directories (scans `.github/workflows/*.y*ml`).
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Policy config (TOML). Omitted → built-in estate default
    /// (anchors: actions, github, hyperpolymath).
    #[arg(long)]
    config: Option<PathBuf>,

    /// JSON output instead of the human tree.
    #[arg(long)]
    json: bool,

    /// Do not fetch action definitions; closures are not expanded.
    #[arg(long)]
    offline: bool,

    /// Max transitive depth.
    #[arg(long, default_value_t = 6)]
    max_depth: usize,

    /// Network timeout (seconds) per fetch.
    #[arg(long, default_value_t = 10)]
    timeout: u64,
}

/// Best-effort repo root for resolving `./` local actions: nearest
/// ancestor containing `.github`, else the file's parent.
fn repo_root_for(p: &Path) -> PathBuf {
    if p.is_dir() {
        return p.to_path_buf();
    }
    let mut cur = p.parent();
    while let Some(d) = cur {
        if d.join(".github").is_dir() {
            return d.to_path_buf();
        }
        cur = d.parent();
    }
    p.parent().unwrap_or(Path::new(".")).to_path_buf()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let cfg = match &cli.config {
        Some(p) => Config::load(p)
            .with_context(|| format!("loading config {}", p.display()))?,
        None => Config::default(),
    };

    let http = HttpResolver {
        timeout_secs: cli.timeout,
    };
    let offline = OfflineResolver;
    let resolver: &dyn Resolver = if cli.offline { &offline } else { &http };

    let mut reports = Vec::new();
    for path in &cli.paths {
        for wf in resolve::discover_workflows(path) {
            let text = std::fs::read_to_string(&wf)
                .with_context(|| format!("reading {}", wf.display()))?;
            let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("warn: skipping {} ({e})", wf.display());
                    continue;
                }
            };
            let root = repo_root_for(&wf);
            let mut roots = Vec::new();
            for raw in parse::workflow_uses(&doc) {
                let mut visiting = HashSet::new();
                roots.push(resolve::walk(
                    &raw,
                    &cfg,
                    resolver,
                    &root,
                    None,
                    0,
                    cli.max_depth,
                    &mut visiting,
                ));
            }
            reports.push(WorkflowReport {
                workflow: wf.display().to_string(),
                roots,
            });
        }
    }

    let summary = Summary::new(reports);
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print!("{}", summary.to_text());
    }
    // Non-zero exit when the layered policy denies anything.
    if summary.denied > 0 {
        std::process::exit(1);
    }
    Ok(())
}
