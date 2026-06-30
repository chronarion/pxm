//! Shell command execution for the agent, plus a small hard denylist.
//!
//! Auto-execute is the chosen default, but "let an LLM run sudo unattended"
//! still gets one guardrail: a handful of catastrophic patterns are refused
//! outright and reported back to the model so it can choose another path.

use anyhow::Result;
use std::process::Command;

pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

/// Patterns that are never executed, regardless of mode. Not a security
/// boundary — just a backstop against the obviously irreversible.
const DENY: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    ":(){:|:&};:",
    "mkfs",
    "dd if=",
    "of=/dev/sd",
    "> /dev/sd",
    "chmod -r 000 /",
    "chown -r root /",
    "format c:",
    "del /s /q c:\\",
    "rmdir /s /q c:\\",
    "shutdown",
    "reboot",
    "mkfs.ext",
];

/// If the command matches a denied pattern, return that pattern.
pub fn is_dangerous(cmd: &str) -> Option<&'static str> {
    let lc = cmd.to_lowercase();
    DENY.iter().copied().find(|pat| lc.contains(pat))
}

/// Run a command through the platform shell and capture its output.
pub fn run(cmd: &str) -> Result<CommandResult> {
    let output = if cfg!(windows) {
        Command::new("cmd").arg("/C").arg(cmd).output()?
    } else {
        Command::new("sh").arg("-c").arg(cmd).output()?
    };
    Ok(CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code().unwrap_or(-1),
    })
}
