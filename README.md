# pxm

**Reliable software installation for Linux. Prompts are packages.**

pxm does not talk to a model itself. It resolves a versioned, dependency-locked
install prompt, composes the final prompt, and hands it to whatever coding agent
you already have installed — **Claude Code**, **Codex**, **Gemini CLI**,
**opencode**, **aider**. pxm is a prompt courier with a lockfile. The registry of
install prompts is embedded in the binary, so there is nothing to fetch on first
run.

```
pxm add postgres-install      # resolve deps, write pxm.lock
pxm run postgres-install      # hand the prompt to your agent; it installs Postgres
```

## Build

Needs a Rust toolchain (`cargo`). Produces a single static binary.

```bash
cargo build --release
# binary at target/release/pxm  (pxm.exe on Windows)
```

## Use

```bash
pxm doctor                    # which coding agents pxm can find on PATH
pxm search redis              # search the embedded registry
pxm info postgres-install     # manifest, dependencies, changelog
pxm add postgres-install      # resolve + lock
pxm list                      # show what's locked
pxm run ripgrep-install       # hand off to your agent and actually install
pxm run postgres-install --dry-run   # print the composed prompt + the command
```

There is no API key to set. Authentication, models, and command execution are
the agent's job — pxm only delivers the prompt.

### Choosing the agent

By default pxm prefers the agent matching the prompt's `[model]` provider pin
(an `anthropic/...` pin prefers Claude Code), then falls back to the first agent
found on PATH. Override it:

```bash
pxm run postgres-install --harness codex
PXM_HARNESS=gemini pxm run postgres-install
pxm run postgres-install --model claude-fable-5   # forwarded to the agent
```

`--no-auto` omits the agent's run-without-asking flag, so it will pause for
approvals instead of installing unattended.

## How a prompt package looks

```
postgres-install/
├── pxm.toml        # name, version, [model] pin, [dependencies]
├── prompt.md       # the install instructions (+ per-provider blocks)
└── CHANGELOG.md
```

`prompt.md` is common text followed by optional `# provider: <vendor>/<model>`
blocks; `pxm run` keeps the common part plus the block matching the agent it
picked.

## Auto-execute

`pxm run` passes the agent's "don't ask, just run" flag by default
(`--dangerously-skip-permissions` for Claude Code, `--yolo` for Gemini, and so
on). That means an agent running `sudo` on your machine unattended. Use a box you
trust it with, or pass `--no-auto`. `--dry-run` runs nothing at all.

## Supported agents

| Agent | Executable | Install |
|-------|-----------|---------|
| Claude Code | `claude`   | `npm i -g @anthropic-ai/claude-code` |
| Codex       | `codex`    | `npm i -g @openai/codex` |
| Gemini CLI  | `gemini`   | `npm i -g @google/gemini-cli` |
| opencode    | `opencode` | https://opencode.ai |
| aider       | `aider`    | `pipx install aider-chat` |

## License

Apache-2.0.
