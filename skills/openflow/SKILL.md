---
name: openflow
description: Use Openflow to plan, run, verify, resume, and report dynamic Codex workflows when a user asks for a workflow, parallel agents, multi-stage audit, migration, review, or verified fan-out/fan-in task.
---

# Openflow

> **When to use:** Use this skill when the user asks for a "workflow", "dynamic workflow", "parallel agents", "fan out", "multi-agent audit", "verified review", "migration plan", or any broad task that benefits from planned stages and independent verification.

## Preconditions

1. Check that `openflow` is installed with `openflow --help`.
2. Check that Codex CLI is installed and authenticated with `codex --version`.
3. Work from the target repository root.
4. For write workflows, make sure the repo is a git repository and inspect `git status --short` before running.

## Default UX

For read-heavy audits or reviews, run a single command:

```bash
openflow run "workflow: <user request>" --template audit --concurrency 6
```

For implementation or migration work, use the staged approval flow:

```bash
openflow plan "workflow: <user request>" --template migration
openflow approve
openflow resume
openflow report --print
```

## Template Selection

- Use `--template audit` for bug hunts, security reviews, dead-code searches, architecture audits, and broad repo inspection.
- Use `--template pr-review` for branch, diff, pull request, or uncommitted-change review.
- Use `--template migration` for dependency, framework, API, architecture, or code movement tasks.

## Operating Rules

1. Prefer read-only workflows unless the user clearly wants code changes.
2. Use `openflow plan` first when a workflow includes write tasks, expensive scope, or uncertain blast radius.
3. Read `openflow status` before resuming a run.
4. Use `openflow report --print` to summarize results back to the user.
5. Do not apply patches automatically unless the user asked for that. If applying, run `openflow apply` and report what changed.
6. If a run fails, inspect `.openflow/runs/<run-id>/planner.log`, task `worker.log`, and verifier logs before retrying.

## Output Contract

When reporting back to the user, include:

- run id
- final status
- report path
- high-signal findings or changed files
- whether patches were only generated or actually applied
- verification command results, if any
