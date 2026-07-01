//! pxm does not run the install itself. It composes a prompt and hands it to
//! whatever coding agent you already have installed — Claude Code, Codex,
//! Gemini CLI, and friends. pxm is, proudly, a prompt courier with a lockfile.

use std::path::PathBuf;

pub struct Harness {
    /// Short id used for `--harness` and `$PXM_HARNESS`.
    pub id: &'static str,
    /// Executable name to look for on PATH.
    pub exe: &'static str,
    /// The pin provider this harness satisfies (so an `anthropic/...` pin
    /// prefers Claude Code). Empty means "provider-agnostic".
    pub provider: &'static str,
    /// The pxm prompt package that installs this agent.
    pub package: &'static str,
}

pub const HARNESSES: &[Harness] = &[
    Harness {
        id: "claude",
        exe: "claude",
        provider: "anthropic",
        package: "claude-install",
    },
    Harness {
        id: "codex",
        exe: "codex",
        provider: "openai",
        package: "codex-install",
    },
    Harness {
        id: "gemini",
        exe: "gemini",
        provider: "google",
        package: "gemini-install",
    },
    Harness {
        id: "opencode",
        exe: "opencode",
        provider: "",
        package: "opencode-install",
    },
    Harness {
        id: "aider",
        exe: "aider",
        provider: "",
        package: "aider-install",
    },
];

pub fn get(id: &str) -> Option<&'static Harness> {
    HARNESSES.iter().find(|h| h.id.eq_ignore_ascii_case(id))
}

/// Locate an executable on PATH, honoring PATHEXT on Windows so that npm shims
/// like `claude.cmd` are found.
pub fn find_on_path(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if cfg!(windows) {
            let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into());
            for ext in exts.split(';') {
                let cand = dir.join(format!("{exe}{}", ext.to_ascii_lowercase()));
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
        let bare = dir.join(exe);
        if bare.is_file() {
            return Some(bare);
        }
    }
    None
}

pub fn detect(h: &Harness) -> Option<PathBuf> {
    find_on_path(h.exe)
}

/// Build the argv to drive a single prompt through a harness, non-interactively.
///
/// `model` is forwarded only when the caller decided it is appropriate (a
/// `claude-*` id makes no sense to Codex). `auto` adds each tool's
/// run-without-asking flag — the chosen default, since the whole point is an
/// unattended install.
pub fn command(h: &Harness, prompt: &str, model: Option<&str>, auto: bool) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    match h.id {
        "claude" => {
            if let Some(m) = model {
                a.push("--model".into());
                a.push(m.into());
            }
            if auto {
                a.push("--dangerously-skip-permissions".into());
            }
            a.push("-p".into());
            a.push(prompt.into());
        }
        "codex" => {
            a.push("exec".into());
            if let Some(m) = model {
                a.push("--model".into());
                a.push(m.into());
            }
            if auto {
                a.push("--dangerously-bypass-approvals-and-sandbox".into());
            }
            a.push(prompt.into());
        }
        "gemini" => {
            if let Some(m) = model {
                a.push("--model".into());
                a.push(m.into());
            }
            if auto {
                a.push("--yolo".into());
            }
            a.push("--prompt".into());
            a.push(prompt.into());
        }
        "opencode" => {
            a.push("run".into());
            if let Some(m) = model {
                a.push("--model".into());
                a.push(m.into());
            }
            a.push(prompt.into());
        }
        "aider" => {
            if let Some(m) = model {
                a.push("--model".into());
                a.push(m.into());
            }
            if auto {
                a.push("--yes-always".into());
            }
            a.push("--message".into());
            a.push(prompt.into());
        }
        _ => a.push(prompt.into()),
    }
    a
}
