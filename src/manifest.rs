//! Parsing for `pxm.toml`, the manifest at the root of every prompt package.

use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub maintainer: String,
    #[serde(default)]
    pub license: String,
    /// The model this prompt was tested against, and what to fall back to.
    #[serde(default)]
    pub model: Option<ModelPin>,
    /// Other prompts this one depends on: name -> version requirement.
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPin {
    pub pin: String,
    #[serde(default)]
    pub fallback: Option<String>,
}

impl Manifest {
    pub fn parse(s: &str) -> Result<Manifest> {
        Ok(toml::from_str(s)?)
    }
}
