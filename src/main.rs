//! pxm — reliable software installation for Linux. Prompts are packages.
//!
//! pxm does not talk to a model directly. It resolves a prompt package, composes
//! the final prompt, and hands it to whatever coding agent you already have
//! installed — Claude Code, Codex, Gemini CLI, and friends. The registry of
//! install prompts is baked into this binary.

mod harness;
mod lockfile;
mod manifest;
mod registry;
mod resolve;
mod ui;

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

/// Instructions wrapped around every install prompt before hand-off. The
/// harness supplies the actual tools; we only describe the job.
const BASE_SYSTEM: &str = "\
You are performing a software installation on the user's machine. Use the tools \
available to you to run the shell commands required. Keep going until the \
software is installed and verified — do not stop to ask for confirmation; you \
have authorization to proceed. Prefer the system package manager. If a command \
fails, read the error and adapt. Before finishing, verify the install actually \
worked (for example, run the program with --version). Then report success in a \
single sentence. The prompt packages below describe how to perform this \
particular installation.";

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
    /// Run an install prompt by handing it to your coding agent.
    Run {
        name: String,
        /// Which agent to use: claude, codex, gemini, opencode, aider.
        #[arg(long)]
        harness: Option<String>,
        /// Override the model passed to the agent.
        #[arg(long)]
        model: Option<String>,
        /// Do not pass the agent's run-without-asking flag.
        #[arg(long)]
        no_auto: bool,
        /// Compose and print the prompt + the command, but run nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Search the registry.
    Search { query: String },
    /// Show the prompts pinned in pxm.lock.
    List,
    /// Show a prompt's manifest, dependencies, and changelog.
    Info { name: String },
    /// Show which coding agents pxm can find on this machine.
    Doctor,
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
        Cmd::Run { name, harness: harness_id, model, no_auto, dry_run } => {
            cmd_run(&name, harness_id, model, no_auto, dry_run)
        }
        Cmd::Search { query } => cmd_search(&query),
        Cmd::List => cmd_list(),
        Cmd::Info { name } => cmd_info(&name),
        Cmd::Doctor => cmd_doctor(),
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

fn cmd_run(
    name: &str,
    harness_override: Option<String>,
    model_override: Option<String>,
    no_auto: bool,
    dry_run: bool,
) -> Result<()> {
    let resolved = resolve::resolve(name)?;
    let target = registry::load(name)?;
    let (pin_provider, pin_model) = pin_parts(&target);
    let auto = !no_auto;

    // Choose a harness. In dry-run we tolerate "none installed".
    let picked: Option<&'static harness::Harness> =
        match pick_harness(&pin_provider, harness_override.as_deref()) {
            Ok(h) => Some(h),
            Err(e) => {
                if !dry_run {
                    return Err(e);
                }
                ui::warn(&e.to_string());
                None
            }
        };

    // Keep the provider block that matches the agent we will actually use;
    // fall back to the package's intended provider for generic agents.
    let provider_for_blocks = match picked {
        Some(h) if !h.provider.is_empty() => h.provider.to_string(),
        _ => pin_provider.clone(),
    };

    let explicit_model =
        model_override.or_else(|| std::env::var("PXM_MODEL").ok().filter(|s| !s.trim().is_empty()));
    let model_to_pass: Option<String> = if let Some(m) = explicit_model {
        Some(m)
    } else if let (Some(h), Some(pm)) = (picked, pin_model.as_ref()) {
        // Only forward the pinned model when the agent is from that provider.
        if !pin_provider.is_empty() && h.provider == pin_provider {
            Some(pm.clone())
        } else {
            None
        }
    } else {
        None
    };

    let system = compose_system(&resolved, &provider_for_blocks)?;
    let task = format!(
        "Install the software described by the prompt package '{name}'. You are running on \
         {os} ({arch}). Run the commands needed to install it. Do not ask for confirmation. \
         When it is installed and verified, stop and report success in one sentence.",
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    );
    let full_prompt = format!("{system}# Task\n{task}\n");

    if dry_run {
        ui::warn("dry-run: nothing will run. Composed prompt:");
        println!("\n{full_prompt}");
        match picked {
            Some(h) => {
                let shown = harness::command(h, "<PROMPT>", model_to_pass.as_deref(), auto);
                ui::info(&format!("would run: {} {}", h.exe, shown.join(" ")));
            }
            None => ui::warn("no coding agent detected on PATH (see `pxm doctor`)."),
        }
        return Ok(());
    }

    let h = picked.expect("pick_harness returns Err when none found and not dry-run");
    let program = harness::detect(h)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| h.exe.to_string());
    let argv = harness::command(h, &full_prompt, model_to_pass.as_deref(), auto);

    ui::step(&format!("Handing off {name} to {}", h.id));
    if auto {
        ui::warn("auto-execute is ON — the agent may run commands without asking.");
    }

    let status = std::process::Command::new(&program)
        .args(&argv)
        .status()
        .map_err(|e| anyhow!("failed to launch {}: {e}", h.id))?;
    if !status.success() {
        bail!("{} exited with status {}", h.id, status.code().unwrap_or(-1));
    }
    ui::ok(&format!("{name} finished."));
    Ok(())
}

/// Split a `[model] pin` into (provider, model). Empty provider if unpinned.
fn pin_parts(target: &registry::Package) -> (String, Option<String>) {
    match &target.manifest.model {
        Some(m) => match m.pin.split_once('/') {
            Some((p, mdl)) => (p.to_string(), Some(mdl.to_string())),
            None => (String::new(), Some(m.pin.clone())),
        },
        None => (String::new(), None),
    }
}

/// Pick a harness: explicit `--harness`/`$PXM_HARNESS`, else the one matching
/// the pin's provider, else the first one found on PATH.
fn pick_harness(
    pin_provider: &str,
    override_id: Option<&str>,
) -> Result<&'static harness::Harness> {
    let chosen = override_id
        .map(|s| s.to_string())
        .or_else(|| std::env::var("PXM_HARNESS").ok().filter(|s| !s.trim().is_empty()));

    if let Some(id) = chosen {
        let h = harness::get(&id)
            .ok_or_else(|| anyhow!("unknown harness '{id}'. Supported: {}", harness_ids()))?;
        if harness::detect(h).is_none() {
            bail!("harness '{}' is not installed.\n  install: {}", h.id, h.install_hint);
        }
        return Ok(h);
    }

    if !pin_provider.is_empty() {
        for h in harness::HARNESSES {
            if h.provider == pin_provider && harness::detect(h).is_some() {
                return Ok(h);
            }
        }
    }
    for h in harness::HARNESSES {
        if harness::detect(h).is_some() {
            return Ok(h);
        }
    }
    bail!(
        "no supported coding agent found on PATH.\n  install one of: {}\n  then re-run (see `pxm doctor`).",
        harness_ids()
    );
}

fn harness_ids() -> String {
    harness::HARNESSES
        .iter()
        .map(|h| h.id)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Assemble the system prompt: base instructions, then each resolved prompt
/// (dependencies first), keeping only the relevant provider block.
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
            keep = !provider.is_empty() && key_provider.eq_ignore_ascii_case(provider);
            continue;
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
        println!("  {} {}  {}", e.name, e.version.green(), format!("sha256:{short}").dimmed());
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

fn cmd_doctor() -> Result<()> {
    println!("{}", "coding agents pxm can hand off to:".bold());
    let mut any = false;
    for h in harness::HARNESSES {
        match harness::detect(h) {
            Some(path) => {
                any = true;
                println!("  {} {:9} {}", "✓".green().bold(), h.id, path.display().to_string().dimmed());
            }
            None => println!(
                "  {} {:9} {}",
                "·".dimmed(),
                h.id,
                format!("not found — {}", h.install_hint).dimmed()
            ),
        }
    }
    if !any {
        println!();
        ui::warn("no coding agent found. Install one above, then `pxm run <prompt>`.");
    }
    Ok(())
}

fn cmd_publish(dir: &str) -> Result<()> {
    let p = std::path::Path::new(dir);
    let toml_src =
        std::fs::read_to_string(p.join("pxm.toml")).map_err(|_| anyhow!("{dir}: missing pxm.toml"))?;
    let m = manifest::Manifest::parse(&toml_src)?;
    std::fs::read_to_string(p.join("prompt.md")).map_err(|_| anyhow!("{dir}: missing prompt.md"))?;
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
