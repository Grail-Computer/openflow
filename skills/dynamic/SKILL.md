---
name: dynamic
description: Viral-friendly alias for Openflow. Use when the user types /dynamic, asks for Codex dynamic workflows, Claude-style dynamic workflows, subagent swarms, verified parallel agents, workflow orchestration, or a broad task that should be planned, run, verified, resumed, and reported through Openflow.
---

# Dynamic

Use this skill as the shortest path into Openflow dynamic workflows.

Openflow is a Rust CLI that turns a broad request into a validated workflow DAG, runs CLI agent workers in parallel, verifies results independently, and writes durable run artifacts under `.openflow/runs/<run-id>/`.

## Trigger

Use this skill when the user says:

- `/dynamic`
- "dynamic workflow"
- "Claude-style workflow"
- "Codex dynamic workflow"
- "swarm"
- "parallel agents"
- "verified multi-agent audit"
- "use Openflow"

## Preconditions

1. Run `openflow --help`.
2. Run `openflow doctor` when checking a fresh install, skill install, or harness setup.
3. If Openflow is missing but this repository is checked out, run `cargo install --path .` from the repo root.
4. For the default Codex preset, run `codex --version`.
5. Work from the target repository root.
6. For write workflows, inspect `git status --short` before running.

## Default Command

For safe read-heavy work:

```bash
openflow run "workflow: <user request>" --template audit --concurrency 6
```

For implementation, migration, large refactors, or anything risky:

```bash
openflow plan "workflow: <user request>" --template migration
openflow approve
openflow resume
openflow report --print
```

## Harnesses

Codex is the default. For any other CLI harness, pass a command template:

```bash
openflow run "workflow: <user request>" \
  --agent custom \
  --agent-command '<agent command using {prompt_file} and optionally {output_file}>'
```

Prefer `{prompt_file}` over `{prompt}` because workflow prompts can be long.

## Operating Rules

1. Keep simple workflows simple; do not add flags unless needed.
2. Use staged `plan -> approve -> resume` for writes, external systems, expensive work, or ambiguous scope.
3. Do not apply generated patches unless the user asks.
4. Use `openflow status` before resuming an interrupted run.
5. Use `openflow report --print` to summarize results.
6. If the workflow fails, inspect `planner.log`, task `worker.log`, verifier JSON, and `report.md`.

## Output Contract

Report back with:

- run id
- status
- report path
- selected harness
- high-signal findings or changed files
- whether patches were only generated or actually applied
- local verification results
