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

## Decision Rule

Use Openflow orchestration when at least two are true:

- The task has independent research, coding, review, migration, QA, docs, or design tracks.
- The task is broad enough that an explicit success contract would reduce drift.
- The task has risk: destructive edits, external writes, deploys, secrets, production data, billing, user accounts, or large repo-wide changes.
- Verification benefits from a separate pass from implementation.
- The workflow could become a reusable recipe or template.
- The user explicitly asks for a dynamic workflow, swarm, parallel agents, or Openflow.

If the task is small, do it directly and mention that full workflow orchestration was unnecessary.

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

## Codex Goal Mode

Codex goal mode is for sustained execution. Use it when the user asks this skill to run a broad workflow end-to-end, or when the workflow will likely require multiple turns, retries, validation, and a final report.

When goal mode tools are available:

1. Start a goal with the full user objective, not just the next shell command.
2. Include the target repo, selected harness, risk gates, expected report, and validation requirement in the goal.
3. Keep working until `openflow validate` passes and the final report has been summarized, or until the workflow is genuinely blocked.
4. Do not enter goal mode for a small one-shot task, a plan-only discussion, or a request that only asks for command examples.

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
5. Use `openflow validate` before presenting a shareable or high-stakes result.
6. Use `openflow report --print` to summarize results.
7. If the workflow fails, inspect `planner.log`, task `worker.log`, verifier JSON, and `report.md`.

## Approval Gates

Prefer `openflow plan` and ask one clear approval question before:

- deleting, overwriting, mass-renaming, force-pushing, or rewriting history
- running migrations, broad codemods, or dependency upgrades
- deploying, publishing, emailing, posting, or changing external systems
- touching credentials, secrets, billing, production data, user accounts, or private customer data
- spawning unusually many agents or running expensive jobs
- making changes outside the requested repository or workspace

If approval is denied or unavailable, continue only with safe read-only planning, local drafts, or non-destructive checks.

## Output Contract

Report back with:

- run id
- status
- report path
- selected harness
- high-signal findings or changed files
- whether patches were only generated or actually applied
- validation status
- local verification results
