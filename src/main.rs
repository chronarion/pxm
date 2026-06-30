//! pxm — reliable software installation for Linux. Prompts are packages.
//!
//! You describe what you want; a coding agent works the problem and verifies
//! the result. Each install prompt is a versioned, dependency-resolved,
//! content-hashed package that lives in a registry embedded in this binary.

mod agent;
mod exec;
mod lockfile;
mod manifest;
mod registry;
mod resolve;
mod ui;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

/// Default Anthropic model when a pin names an unimplemented provider.
const DEFAULT_MODEL: &str = "claude-fable-5";

/// The base system prompt wrapped around every install run.
const BASE_SYSTEM: &str = "\
You are pxm's installation agent. You install software on the user's machine by \
running shell commands through the run_command tool. You auto-execute: run the \
commands yourself, read their output, and keep going until the software is \
installed and verified. You have the user's authorization to proceed — do not \
ask questions or wait for confirmation. Prefer the system package manager. If a \
command fails, read the error and adapt. Before you stop, verify the install \
actually worked (for example, run the program with --version). Then report \
success in a single sentence. The following prompt packages describe how to \
perform this particular installation:";

#[derive(Parser)]
#[command(
    name = "pxm",
    version,
    about = "Reliable software installation for Linux. Prompts are packages."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Resolve a prompt and its dependencies, then write pxm.lock.
    Add { name: String },
    /// Run an install prompt. Drives a coding agent that auto-executes commands.
    Run {
        name: String,
        /// Override the model, e.g. anthropic/claude-fable-5 or just a model id.
        #[arg(long)]
        model: Option<String>,
        /// Compose and print the install prompt without running anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Search the registry.
    Search { query: String },
    /// Show the prompts pinned in pxm.lock.
    List,
    /// Show a prompt's manifest, dependencies, and changelog.
    Info { name: String },
    /// Validate a local prompt package for publishing.
    Publish { dir: String },
    /// Check for newer revisions.
    Upgrade { name: Option<String> },
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Add { name } => cmd_add(&name),
        Cmd::Run { name, model, dry_run } => cmd_run(&name, model, dry_run),
        Cmd::Search { query } => cmd_search(&query),
        Cmd::List => cmd_list(),
        Cmd::Info { name } => cmd_info(&name),
        Cmd::Publish { dir } => cmd_publish(&dir),
        Cmd::Upgrade { name } => cmd_upgrade(name),
    }
}

fn cmd_add(name: &str) -> Result<()> {
    let resolved = resolve::resolve(name)?;
    ui::info("Resolving prompt dependencies...");
    print_tree(name, &resolved)?;
    std::fs::write("pxm.lock", lockfile::build(name, &resolved)?)?;
    ui::ok(&format!("Locked {} prompts.", resolved.order.len()));
    Ok(())
}

fn print_tree(root: &str, resolved: &resolve::Resolved) -> Result<()> {
    let pkg = registry::load(root)?;
    println!("  {}", format!("{}@{}", root, pkg.manifest.version).green());
    let deps: Vec<&String> = pkg.manifest.dependencies.keys().collect();
    for (i, dep) in deps.iter().enumerate() {
        let branch = if i + 1 == deps.len() { "└──" } else { "├──" };
        let ver = resolved.versions.get(*dep).map(String::as_str).unwrap_or("?");
        println!("    {branch} {dep}@{ver}");
    }
    Ok(())
}

fn cmd_run(name: &str, model_override: Option<String>, dry_run: bool) -> Result<()> {
    let resolved = resolve::resolve(name)?;
    let target = registry::load(name)?;

    let spec = resolve_model(&target, model_override);
    let (provider, mut model_id) = split_model(&spec);
    let system = compose_system(&resolved, &provider)?;

    let task = format!(
        "Install the software described by the prompt package '{name}'. You are running on \
         {os} ({arch}). Auto-execute the commands needed via run_command. Do not ask for \
         confirmation. When the software is installed and verified, stop and report success \
         in one sentence.",
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    );

    if dry_run {
        ui::warn("dry-run: no commands will execute. Composed install prompt:");
        println!("\n{system}\n");
        println!("---\n{task}");
        ui::info(&format!("model: {model_id} (provider: {provider})"));
        return Ok(());
    }

    if provider != "anthropic" {
        ui::warn(&format!(
            "provider '{provider}' is not implemented yet; falling back to anthropic/{DEFAULT_MODEL}."
        ));
        model_id = DEFAULT_MODEL.to_string();
    }

    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        anyhow!("ANTHROPIC_API_KEY is not set.\n  Set it and run again:  export ANTHROPIC_API_KEY=sk-ant-...")
    })?;

    ui::step(&format!("Running {name} with {model_id}"));
    ui::warn("auto-execute is ON — the agent will run shell commands on this machine.");

    let cfg = agent::AgentConfig {
        model: model_id,
        api_key,
        max_steps: 40,
    };
    agent::run_install(&system, &task, &cfg)?;
    ui::ok(&format!("{name} finished."));
    Ok(())
}

/// Pick the model: --model, then $PXM_MODEL, then the manifest pin, then default.
fn resolve_model(target: &registry::Package, override_: Option<String>) -> String {
    if let Some(m) = override_ {
        return m;
    }
    if let Ok(m) = std::env::var("PXM_MODEL") {
        if !m.trim().is_empty() {
            return m;
        }
    }
    if let Some(mp) = &target.manifest.model {
        return mp.pin.clone();
    }
    format!("anthropic/{DEFAULT_MODEL}")
}

fn split_model(spec: &str) -> (String, String) {
    match spec.split_once('/') {
        Some((p, m)) => (p.to_string(), m.to_string()),
        None => ("anthropic".to_string(), spec.to_string()),
    }
}

/// Assemble the full system prompt: base instructions, then each resolved
/// prompt (dependencies first), with only the relevant provider block kept.
fn compose_system(resolved: &resolve::Resolved, provider: &str) -> Result<String> {
    let mut s = String::from(BASE_SYSTEM);
    s.push_str("\n\n");
    for name in &resolved.order {
        let pkg = registry::load(name)?;
        s.push_str(&format!("## prompt: {}@{}\n", pkg.manifest.name, pkg.manifest.version));
        s.push_str(select_provider_block(&pkg.prompt, provider).trim());
        s.push_str("\n\n");
    }
    Ok(s)
}

/// A prompt.md is common text followed by optional `# provider: <id>` blocks.
/// Keep the common part plus the block whose provider matches.
fn select_provider_block(prompt: &str, provider: &str) -> String {
    let mut out = String::new();
    let mut keep = true;
    for line in prompt.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("# provider:") {
            let key = rest.trim();
            let key_provider = key.split('/').next().unwrap_or(key).trim();
            keep = key_provider.eq_ignore_ascii_case(provider);
            continue; // never emit the header line itself
        }
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn cmd_search(query: &str) -> Result<()> {
    let results = registry::search(query);
    if results.is_empty() {
        ui::warn("No prompts found. Writing one is straightforward.");
        return Ok(());
    }
    for p in results {
        println!("{} {}", p.manifest.name.bold(), p.manifest.version.green());
        println!("  {}", p.manifest.description.dimmed());
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    let data = std::fs::read_to_string("pxm.lock")
        .map_err(|_| anyhow!("no pxm.lock here. Run `pxm add <prompt>` first."))?;
    let lock: lockfile::Lock = toml::from_str(&data)?;
    println!("root: {}", lock.root.green());
    for e in &lock.prompt {
        let short = &e.sha256[..e.sha256.len().min(12)];
        println!(
            "  {} {}  {}",
            e.name,
            e.version.green(),
            format!("sha256:{short}").dimmed()
        );
    }
    Ok(())
}

fn cmd_info(name: &str) -> Result<()> {
    let pkg = registry::load(name)?;
    println!("{} {}", pkg.manifest.name.bold(), pkg.manifest.version.green());
    if !pkg.manifest.description.is_empty() {
        println!("{}", pkg.manifest.description.dimmed());
    }
    if let Some(m) = &pkg.manifest.model {
        println!("\nmodel pin: {}", m.pin);
        if let Some(f) = &m.fallback {
            println!("fallback:  {f}");
        }
    }
    if !pkg.manifest.dependencies.is_empty() {
        println!("\ndependencies:");
        for (k, v) in &pkg.manifest.dependencies {
            println!("  {} {}", k, v.dimmed());
        }
    }
    if let Some(cl) = &pkg.changelog {
        println!("\n{}\n{}", "CHANGELOG".bold(), cl.trim());
    }
    Ok(())
}

fn cmd_publish(dir: &str) -> Result<()> {
    let p = std::path::Path::new(dir);
    let toml_src = std::fs::read_to_string(p.join("pxm.toml"))
        .map_err(|_| anyhow!("{dir}: missing pxm.toml"))?;
    let m = manifest::Manifest::parse(&toml_src)?;
    std::fs::read_to_string(p.join("prompt.md"))
        .map_err(|_| anyhow!("{dir}: missing prompt.md"))?;
    ui::ok(&format!("validated {}@{}", m.name, m.version));
    ui::info("publishing to the public registry requires two maintainer approvals and a passing distro matrix.");
    ui::info("this build ships an embedded registry; remote publish is not wired up.");
    Ok(())
}

fn cmd_upgrade(name: Option<String>) -> Result<()> {
    match name {
        Some(n) => ui::ok(&format!("{n} is at the latest revision in the embedded registry.")),
        None => ui::ok("All prompts are at the latest revision in the embedded registry."),
    }
    Ok(())
}
