//! Terminal output helpers. Deadpan, lightly colored.

use crate::exec::CommandResult;
use owo_colors::OwoColorize;

pub fn step(msg: &str) {
    println!("{} {}", "▸".green().bold(), msg.bold());
}

pub fn info(msg: &str) {
    println!("  {}", msg.dimmed());
}

pub fn warn(msg: &str) {
    println!("{} {}", "!".yellow().bold(), msg.yellow());
}

pub fn ok(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

pub fn agent_say(msg: &str) {
    println!("{} {}", "agent:".cyan().bold(), msg);
}

pub fn command(cmd: &str, why: &str) {
    if why.trim().is_empty() {
        println!("{} {}", "$".green().bold(), cmd.bold());
    } else {
        println!(
            "{} {}  {}",
            "$".green().bold(),
            cmd.bold(),
            format!("# {why}").dimmed()
        );
    }
}

pub fn command_output(r: &CommandResult) {
    let body = if !r.stdout.trim().is_empty() {
        r.stdout.trim()
    } else {
        r.stderr.trim()
    };
    for line in body.lines().take(12) {
        println!("  {}", line.dimmed());
    }
    if r.code != 0 {
        println!("  {} exit {}", "✗".red().bold(), r.code);
    }
}

pub fn refused(pattern: &str) {
    println!(
        "  {} refused by safety guard (matches \"{}\")",
        "✗".red().bold(),
        pattern
    );
}
