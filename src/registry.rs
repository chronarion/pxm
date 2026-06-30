//! The prompt registry, embedded into the binary at compile time.
//!
//! Committing to the bit: the "single static binary" really does carry the
//! registry inside it. `include_dir!` bakes the `registry/` tree into the
//! executable, so `pxm search` works with no network and no first-run fetch.

use crate::manifest::Manifest;
use anyhow::{anyhow, Result};
use include_dir::{include_dir, Dir};

static REGISTRY: Dir = include_dir!("$CARGO_MANIFEST_DIR/registry");

pub struct Package {
    pub manifest: Manifest,
    pub prompt: String,
    pub changelog: Option<String>,
}

/// Load a single prompt package by name.
pub fn load(name: &str) -> Result<Package> {
    let toml_path = format!("{name}/pxm.toml");
    let prompt_path = format!("{name}/prompt.md");

    let toml_src = REGISTRY
        .get_file(&toml_path)
        .ok_or_else(|| anyhow!("prompt '{name}' not found in registry"))?
        .contents_utf8()
        .ok_or_else(|| anyhow!("{name}: pxm.toml is not valid UTF-8"))?;

    let prompt = REGISTRY
        .get_file(&prompt_path)
        .ok_or_else(|| anyhow!("{name}: missing prompt.md"))?
        .contents_utf8()
        .ok_or_else(|| anyhow!("{name}: prompt.md is not valid UTF-8"))?
        .to_string();

    let changelog = REGISTRY
        .get_file(&format!("{name}/CHANGELOG.md"))
        .and_then(|f| f.contents_utf8())
        .map(|s| s.to_string());

    Ok(Package {
        manifest: Manifest::parse(toml_src)?,
        prompt,
        changelog,
    })
}

/// Every package in the registry.
pub fn all() -> Vec<Package> {
    REGISTRY
        .dirs()
        .filter_map(|d| {
            let name = d.path().file_name()?.to_str()?;
            load(name).ok()
        })
        .collect()
}

/// Substring search across names and descriptions.
pub fn search(query: &str) -> Vec<Package> {
    let q = query.to_lowercase();
    let mut hits: Vec<Package> = all()
        .into_iter()
        .filter(|p| {
            p.manifest.name.to_lowercase().contains(&q)
                || p.manifest.description.to_lowercase().contains(&q)
        })
        .collect();
    hits.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    hits
}
