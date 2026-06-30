//! The install agent: a tool-use loop against the Anthropic Messages API.
//!
//! The model is given exactly one tool — `run_command` — and told to install
//! the software. We execute whatever it asks for (subject to the denylist in
//! `exec`), feed the result back, and loop until it stops calling tools.

use crate::{exec, ui};
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub struct AgentConfig {
    pub model: String,
    pub api_key: String,
    pub max_steps: usize,
}

pub fn run_install(system: &str, task: &str, cfg: &AgentConfig) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    let tools = json!([{
        "name": "run_command",
        "description": "Run a shell command on the user's machine and return its stdout, stderr, and exit code.",
        "input_schema": {
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The exact shell command to run." },
                "explanation": { "type": "string", "description": "One short sentence on why you are running it." }
            },
            "required": ["command"]
        }
    }]);

    let mut messages: Vec<Value> = vec![json!({ "role": "user", "content": task })];

    for _ in 0..cfg.max_steps {
        let body = json!({
            "model": cfg.model,
            "max_tokens": 4096,
            "system": system,
            "tools": tools,
            "messages": messages,
        });

        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &cfg.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()?;

        let status = resp.status();
        let v: Value = resp.json()?;
        if !status.is_success() {
            let detail = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            bail!("Anthropic API error ({}): {}", status.as_u16(), detail);
        }

        let content = v
            .get("content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        let stop_reason = v.get("stop_reason").and_then(|s| s.as_str()).unwrap_or("");

        // Surface any prose the model emitted this turn.
        for block in &content {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                    if !t.trim().is_empty() {
                        ui::agent_say(t.trim());
                    }
                }
            }
        }

        // Owned copies of the tool_use blocks, so we can move `content` into
        // the assistant message without fighting the borrow checker.
        let tool_uses: Vec<Value> = content
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .cloned()
            .collect();

        if tool_uses.is_empty() || stop_reason != "tool_use" {
            return Ok(());
        }

        messages.push(json!({ "role": "assistant", "content": content }));

        let mut results = Vec::new();
        for tu in &tool_uses {
            let id = tu.get("id").and_then(|i| i.as_str()).unwrap_or_default();
            let input = tu.get("input").cloned().unwrap_or_else(|| json!({}));
            let cmd = input.get("command").and_then(|c| c.as_str()).unwrap_or_default();
            let why = input.get("explanation").and_then(|c| c.as_str()).unwrap_or_default();

            ui::command(cmd, why);

            let result_text = if let Some(pat) = exec::is_dangerous(cmd) {
                ui::refused(pat);
                format!("REFUSED by pxm safety guard: command matches the dangerous pattern \"{pat}\" and was not executed. Choose a safer approach.")
            } else {
                match exec::run(cmd) {
                    Ok(r) => {
                        ui::command_output(&r);
                        render_result(&r)
                    }
                    Err(e) => format!("pxm failed to spawn the command: {e}"),
                }
            };

            results.push(json!({
                "type": "tool_result",
                "tool_use_id": id,
                "content": result_text,
            }));
        }

        messages.push(json!({ "role": "user", "content": results }));
    }

    bail!("reached the step limit ({}) without finishing", cfg.max_steps);
}

fn render_result(r: &exec::CommandResult) -> String {
    let mut s = format!("exit_code: {}\n", r.code);
    if !r.stdout.trim().is_empty() {
        s.push_str(&format!("stdout:\n{}\n", clamp(&r.stdout, 6000)));
    }
    if !r.stderr.trim().is_empty() {
        s.push_str(&format!("stderr:\n{}\n", clamp(&r.stderr, 4000)));
    }
    s
}

/// Truncate on a char boundary so we never split a UTF-8 sequence.
fn clamp(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let kept: String = s.chars().take(max_chars).collect();
    format!("{kept}\n...[truncated]")
}
