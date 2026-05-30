# Openflow

Open-source dynamic workflow orchestration for CLI agent harnesses.

Openflow turns one broad request into a validated workflow DAG, runs agent workers in parallel, verifies results independently, and saves the whole run as ordinary files so it can be inspected, resumed, and shared.

It is written in Rust. Codex is the default built-in runner, but Openflow is not Codex-specific. Any agent harness that can run non-interactively from the CLI can be plugged in.

## Why

Single-agent coding works well for focused tasks. It gets weaker when the request naturally has stages:

- inspect several subsystems
- compare independent findings
- verify each result before trusting it
- synthesize the final answer
- resume after interruption

Openflow adds that missing orchestration layer.

## Install

Prerequisites:

- Rust toolchain
- A CLI agent harness installed and authenticated. Codex works out of the box.
- Git for patch/worktree workflows

Install from GitHub:

```bash
cargo install --git https://github.com/Grail-Computer/openflow openflow
```

Install from a local checkout:

```bash
git clone https://github.com/Grail-Computer/openflow
cd openflow
cargo install --path .
```

Check it:

```bash
openflow --help
```

Install the Codex skill shortcut:

```bash
openflow install-skill
```

This installs the `dynamic` skill into `$CODEX_HOME/skills` or `~/.codex/skills`.
Restart Codex, then invoke it with `Use $dynamic ...` or `/dynamic` if your client exposes skill shortcuts.

## Quick Start

Run a read-only audit:

```bash
cd your-repo
openflow run "workflow: audit this repo for auth and permission bugs" --template audit --concurrency 8
```

This uses the built-in Codex preset, equivalent to running `codex exec` workers under Openflow's planner/verifier orchestration.

Use another agent harness:

```bash
openflow run "workflow: audit this repo for auth and permission bugs" \
  --template audit \
  --agent kimi-k2 \
  --agent-command 'kimi-k2-cli run --prompt-file {prompt_file} --output {output_file}' \
  --concurrency 8
```

If your harness writes the final answer to stdout, Openflow will capture stdout into the expected output file:

```bash
openflow run "workflow: review this repo for risky changes" \
  --agent my-agent \
  --agent-command 'my-agent --prompt-file {prompt_file}'
```

Safer staged flow:

```bash
openflow plan "workflow: migrate this app from Next 14 to Next 15" --template migration
openflow approve
openflow resume
openflow report --print
```

Initialize editable project templates:

```bash
openflow init
```

This creates:

```text
.openflow/workflows/audit.md
.openflow/workflows/migration.md
.openflow/workflows/pr-review.md
```

## Commands

```bash
openflow init
openflow templates
openflow plan "workflow: ..." --template audit
openflow run "workflow: ..." --template audit --concurrency 8
openflow approve [run-id]
openflow resume [run-id]
openflow status [run-id]
openflow report [run-id] --print
openflow apply [run-id]
openflow install-skill [--name dynamic|openflow|all]
```

Useful options:

```bash
--template <name>          audit, migration, pr-review
--concurrency <n>          max concurrent agent workers
--max-retries <n>          verifier-driven retries per task
--model <model>            passed to the selected agent preset/harness
--agent <name>             agent preset/name, default: codex
--agent-bin <path>         executable for built-in presets, default: codex
--agent-command <template> custom shell command for any harness
--skip-git-repo-check      passed to the Codex preset
--yes                      skip approval prompt
```

## Agent Harness Contract

Openflow has one built-in preset today:

```bash
--agent codex --agent-bin codex
```

For everything else, use `--agent-command`.

The command runs once per planner, worker, and verifier task. Openflow provides these shell-quoted placeholders:

```text
{prompt}        full prompt text
{prompt_file}   path to a file containing the prompt
{output_file}   path where the agent should write its final answer
{schema_file}   JSON schema path when Openflow expects structured JSON
{sandbox}       read-only or workspace-write
{cwd}           working directory
{model}         model value passed with --model, if any
{agent}         agent name
{agent_bin}     agent binary path/name
```

Openflow also sets environment variables for custom commands:

```text
OPENFLOW_AGENT
OPENFLOW_PROMPT
OPENFLOW_PROMPT_FILE
OPENFLOW_OUTPUT_FILE
OPENFLOW_SCHEMA_FILE
OPENFLOW_SANDBOX
OPENFLOW_MODEL
```

The harness must return the final agent response either by writing `{output_file}` or by printing to stdout. Planner and verifier calls must return JSON because Openflow validates and consumes those outputs.

Examples:

```bash
# Prompt file + explicit output file
--agent-command 'my-agent run --prompt-file {prompt_file} --output {output_file}'

# Stdout capture
--agent-command 'my-agent run --prompt-file {prompt_file}'

# Harness that supports schema guidance
--agent-command 'my-agent run --prompt-file {prompt_file} --schema {schema_file} --output {output_file}'
```

## Defaults And Per-Step Overrides

Openflow is designed to be simple first:

```bash
openflow run "workflow: audit this repo for auth bugs"
```

If you do nothing else, every planner, worker, and verifier uses the same runtime defaults from the CLI. Today that means the Codex preset, read-only sandboxes for read tasks, workspace-write sandboxes for write tasks, one verifier, and one retry.

When you want more control, edit the generated plan:

```bash
openflow plan "workflow: audit this repo and use a stronger model for security review"
$EDITOR .openflow/runs/<run-id>/plan.json
openflow approve <run-id>
openflow resume <run-id>
```

Plan-level defaults apply to every task unless a task overrides them:

```json
{
  "defaults": {
    "agent": "codex",
    "agentBin": "codex",
    "model": "gpt-5",
    "sandbox": "read-only",
    "writeSandbox": "workspace-write",
    "verifierModel": "gpt-5"
  }
}
```

Each task can override the runtime without changing the rest of the workflow:

```json
{
  "id": "audit-auth-boundaries",
  "title": "Audit auth boundaries",
  "kind": "explore",
  "role": "security-reviewer",
  "model": "gpt-5-high",
  "sandbox": "read-only",
  "verifiersPerTask": 2,
  "verifierModel": "gpt-5",
  "maxRetries": 2,
  "prompt": "Inspect auth and tenant isolation boundaries."
}
```

Per-task fields:

```text
role                    human-readable worker role
agent                   harness preset/name override
agentBin                executable override for built-in presets
agentCommand            custom command template override
model                   model override for this worker
sandbox                 read-only, workspace-write, or danger-full-access
maxRetries              retry limit for this task
verifiersPerTask        verifier count for this task
verificationPrompt      verifier instructions for this task
verifierAgent           verifier harness override
verifierAgentBin        verifier executable override
verifierAgentCommand    verifier command template override
verifierModel           verifier model override
verifierSandbox         verifier sandbox override
```

Precedence is:

```text
built-in/CLI defaults -> workflow defaults -> task overrides
```

## How It Works

1. Planner phase: Openflow asks the selected agent harness for a strict JSON workflow plan in a read-only sandbox.
2. Validation phase: Openflow validates task ids, dependencies, write flags, task kinds, and dependency cycles.
3. Execution phase: ready tasks run concurrently as separate agent workers.
4. Isolation phase: write tasks run in per-task git worktrees.
5. Verification phase: independent verifier workers accept, reject, or request revision.
6. Retry phase: verifier feedback is sent back into the worker up to `--max-retries`.
7. Report phase: Openflow writes a deterministic `report.md`.
8. Resume phase: every run persists to `.openflow/runs/<run-id>/state.json`.

If a task fails or the workflow becomes blocked, Openflow still writes the report first and then exits nonzero.

## Run State

Every run is inspectable:

```text
.openflow/
  runs/
    <run-id>/
      state.json
      plan.json
      planner-prompt.md
      planner.log
      schemas/
      tasks/
        <task-id>/
          attempt-1/
            prompt.md
            worker.log
            result.md
            verifier-1.json
            verifier-1.log
      patches/
      report.md
```

No hidden service. No opaque database. No background daemon.

## Safety Model

With the default Codex preset, read-only tasks run through:

```bash
codex exec --sandbox read-only
```

With the default Codex preset, write tasks run through:

```bash
codex exec --sandbox workspace-write
```

But write tasks are isolated in git worktrees:

```text
.openflow/runs/<run-id>/worktrees/<task-id>/
```

Patches are captured here:

```text
.openflow/runs/<run-id>/patches/<task-id>.diff
```

`openflow apply` checks every patch with `git apply --check` before applying anything to the main workspace.

## Example

```bash
openflow run \
  "workflow: find real security bugs in this repo. Focus on auth, tenant isolation, and webhook handling." \
  --template audit \
  --concurrency 6
```

The planner might create tasks like:

- `map-auth-entrypoints`
- `audit-session-validation`
- `audit-tenant-boundaries`
- `audit-webhook-signatures`
- `review-test-coverage`
- `synthesize-risk-report`

Each task gets its own worker prompt. Each result is checked by a verifier before it lands in the final report.

## Agent Skill

This repo includes Codex skills:

- [skills/dynamic/SKILL.md](skills/dynamic/SKILL.md): short, viral-friendly entry point for dynamic workflows.
- [skills/openflow/SKILL.md](skills/openflow/SKILL.md): explicit Openflow operator skill with more direct CLI guidance.

Install the default `dynamic` shortcut:

```bash
openflow install-skill
```

Install both skills:

```bash
openflow install-skill --name all
```

Then ask Codex for a workflow:

```text
Use $dynamic to run a workflow that audits this repo for auth bugs.
```

If your Codex client exposes skill shortcuts, invoke:

```text
/dynamic
```

Or:

```text
Use Openflow with this harness command: kimi-k2-cli run --prompt-file {prompt_file} --output {output_file}. Run a workflow that audits this repo for auth bugs.
```

## Docs

- [Full user experience map](docs/USER_EXPERIENCE.md)
- [YouTube demo plan](docs/YOUTUBE_DEMO_PLAN.md)
- [Publish and leak-safety checklist](docs/PUBLISH_CHECKLIST.md)

## Current Limits

- Openflow is strongest today for read-heavy audit, review, and migration planning workflows.
- Write workflows produce patch queues; conflict resolution is still manual.
- It shells out to your selected agent harness, so that harness's auth, sandboxing, and rate limits apply.
- Very large workflows can spend a lot of tokens. Start scoped.

## Development

```bash
./scripts/check.sh
```

The local check script runs:

```bash
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps --document-private-items
```

The test suite includes fake built-in and custom agent harnesses so the planner, runner, verifier, state, and report plumbing can be tested without spending real agent tokens.

## Roadmap

- Live terminal DAG view
- GitHub Action mode that uploads reports and patch artifacts
- More built-in presets for other agent harnesses
- Native JSONL event ingestion for harnesses that support event streams
- Better patch merge and conflict handling
- Shared workflow template registry
- Machine-readable final report mode

## Positioning

Claude popularized "dynamic workflows" for agent-written orchestration. Openflow is the transparent, hackable, open-source version for any CLI agent harness, with Codex supported out of the box.
