# Openflow

Open-source dynamic workflow orchestration for Codex CLI.

Openflow turns one broad request into a validated workflow DAG, runs Codex workers in parallel, verifies results independently, and saves the whole run as ordinary files so it can be inspected, resumed, and shared.

It is written in Rust and intentionally stays close to Codex itself: Openflow does not replace Codex. It orchestrates `codex exec`.

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
- Codex CLI installed and authenticated
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

## Quick Start

Run a read-only audit:

```bash
cd your-repo
openflow run "workflow: audit this repo for auth and permission bugs" --template audit --concurrency 8
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
```

Useful options:

```bash
--template <name>          audit, migration, pr-review
--concurrency <n>          max concurrent Codex workers
--max-retries <n>          verifier-driven retries per task
--model <model>            passed to codex exec
--codex-bin <path>         alternate Codex executable
--skip-git-repo-check      passed to codex exec
--yes                      skip approval prompt
```

## How It Works

1. Planner phase: Openflow asks `codex exec` for a strict JSON workflow plan in a read-only sandbox.
2. Validation phase: Openflow validates task ids, dependencies, write flags, task kinds, and dependency cycles.
3. Execution phase: ready tasks run concurrently as separate `codex exec` workers.
4. Isolation phase: write tasks run in per-task git worktrees.
5. Verification phase: independent verifier workers accept, reject, or request revision.
6. Retry phase: verifier feedback is sent back into the worker up to `--max-retries`.
7. Report phase: Openflow writes a deterministic `report.md`.
8. Resume phase: every run persists to `.openflow/runs/<run-id>/state.json`.

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

Read-only tasks run through:

```bash
codex exec --sandbox read-only
```

Write tasks run through:

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

Each task gets its own Codex worker prompt. Each result is checked by a verifier before it lands in the final report.

## Codex Skill

This repo includes an optional Codex skill at [skills/openflow/SKILL.md](skills/openflow/SKILL.md).

Install it into your Codex skills folder:

```bash
mkdir -p ~/.codex/skills/openflow
curl -fsSL https://raw.githubusercontent.com/Grail-Computer/openflow/main/skills/openflow/SKILL.md \
  -o ~/.codex/skills/openflow/SKILL.md
```

Then ask Codex for a workflow:

```text
Use Openflow to run a workflow that audits this repo for auth bugs.
```

## Docs

- [Full user experience map](docs/USER_EXPERIENCE.md)
- [YouTube demo plan](docs/YOUTUBE_DEMO_PLAN.md)
- [Publish and leak-safety checklist](docs/PUBLISH_CHECKLIST.md)

## Current Limits

- Openflow is strongest today for read-heavy audit, review, and migration planning workflows.
- Write workflows produce patch queues; conflict resolution is still manual.
- It shells out to `codex exec`, so your Codex auth, sandboxing, and rate limits apply.
- Very large workflows can spend a lot of tokens. Start scoped.

## Development

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features
```

The test suite includes a fake Codex executable path so the planner, runner, verifier, state, and report plumbing can be tested without spending Codex tokens.

## Roadmap

- Live terminal DAG view
- GitHub Action mode that uploads reports and patch artifacts
- Native JSONL event ingestion from `codex exec --json`
- Better patch merge and conflict handling
- Shared workflow template registry
- Machine-readable final report mode

## Positioning

Claude popularized "dynamic workflows" for agent-written orchestration. Openflow is the transparent, hackable, open-source version for Codex users.
