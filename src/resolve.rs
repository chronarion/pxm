//! Prompt dependency resolution: a depth-first topological sort over the
//! registry, with cycle detection. Dependencies come out before the prompts
//! that need them, which is also the order we feed them to the agent.

use crate::registry;
use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};

pub struct Resolved {
    /// Topological order: dependencies first, the requested prompt last.
    pub order: Vec<String>,
    /// Resolved name -> version for everything in `order`.
    pub versions: BTreeMap<String, String>,
}

pub fn resolve(root: &str) -> Result<Resolved> {
    let mut versions = BTreeMap::new();
    let mut order = Vec::new();
    let mut on_stack = BTreeSet::new();
    let mut done = BTreeSet::new();
    visit(root, &mut versions, &mut order, &mut on_stack, &mut done)?;
    Ok(Resolved { order, versions })
}

fn visit(
    name: &str,
    versions: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
    on_stack: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
) -> Result<()> {
    if done.contains(name) {
        return Ok(());
    }
    if !on_stack.insert(name.to_string()) {
        bail!("dependency cycle detected at '{name}'");
    }

    let pkg = registry::load(name)?;
    for dep in pkg.manifest.dependencies.keys() {
        visit(dep, versions, order, on_stack, done)?;
    }

    on_stack.remove(name);
    done.insert(name.to_string());
    versions.insert(name.to_string(), pkg.manifest.version.clone());
    order.push(name.to_string());
    Ok(())
}
