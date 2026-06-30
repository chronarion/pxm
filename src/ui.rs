//! Terminal output helpers. Deadpan, lightly colored.

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
