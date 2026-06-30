# pxm

**Reliable software installation for Linux. Prompts are packages.**

You describe what you want; a coding agent works the problem and verifies the
result. Each install prompt is a versioned, dependency-resolved, content-hashed
package. The registry is embedded in the binary, so there is nothing to fetch on
first run.

```
pxm add postgres-install      # resolve deps, write pxm.lock
pxm run postgres-install      # drive the agent; it installs Postgres
```

## Build

Needs a Rust toolchain (`cargo`). Produces a single static binary.

```bash
cargo build --release
# binary at target/release/pxm  (pxm.exe on Windows)
```

## Use

```bash
pxm search redis              # search the embedded registry
pxm info postgres-install     # manifest, dependencies, changelog
pxm add postgres-install      # resolve + lock
pxm list                      # show what's locked
pxm run ripgrep-install       # actually install something (auto-execute)
pxm run postgres-install --dry-run   # compose the prompt, run nothing
```

`pxm run` calls the Anthropic API, so set a key:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

Override the model with `--model` or `$PXM_MODEL` (e.g. `--model
anthropic/claude-fable-5`). The model is otherwise taken from the prompt
package's `[model]` pin in `pxm.toml`.

## How a prompt package looks

```
postgres-install/
├── pxm.toml        # name, version, [model] pin, [dependencies]
├── prompt.md       # the install instructions (+ per-provider blocks)
└── CHANGELOG.md
```

`prompt.md` is common text followed by optional `# provider: <vendor>/<model>`
blocks; `pxm run` keeps the common part plus the block matching the resolved
provider.

## Auto-execute and safety

`pxm run` auto-executes the commands the agent proposes — no per-command
confirmation. A small hard denylist (`src/exec.rs`) refuses the obviously
catastrophic (`rm -rf /`, `mkfs`, `shutdown`, …) and reports the refusal back to
the model. It is a backstop, not a security boundary. Run it on a machine you
are willing to let an LLM type `sudo` into. `--dry-run` executes nothing.

## License

Apache-2.0.
