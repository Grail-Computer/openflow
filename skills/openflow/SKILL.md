---
name: openflow
description: Use Openflow to plan, run, verify, resume, and report dynamic workflows with Codex or any non-interactive CLI agent harness when a user asks for a workflow, parallel agents, multi-stage audit, migration, review, or verified fan-out/fan-in task.
---

# Openflow

> **When to use:** Use this skill when the user asks for a "workflow", "dynamic workflow", "parallel agents", "fan out", "multi-agent audit", "verified review", "migration plan", or any broad task that benefits from planned stages and independent verification.

## Preconditions

1. Check that `openflow` is installed with `openflow --help`.
2. Identify the target harness. Use the default Codex preset unless the user provides another harness command.
3. For the Codex preset, check `codex --version`. For a custom harness, check the binary or command the user gave.
4. Work from the target repository root.
5. For write workflows, make sure the repo is a git repository and inspect `git status --short` before running.

## Default UX

For read-heavy audits or reviews, run a single command:

```bash
openflow run "workflow: <user request>" --template audit --concurrency 6
```

For a custom harness, pass the command template:

```bash
openflow run "workflow: <user request>" \
  --template audit \
  --agent custom \
  --agent-command '<agent command using {prompt_file} and optionally {output_file}>'
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

## Harness Selection

- Use the default Codex preset when the user does not specify a harness.
- Use `--agent <name> --agent-command '<template>'` when the user gives a Kimi/K2, Claude, Aider, local model, or other CLI harness.
- Prefer `{prompt_file}` over `{prompt}` for custom harnesses because workflow prompts can be long.
- If the harness can write to a file, include `{output_file}`. If it only prints to stdout, Openflow will capture stdout.
- If the harness supports structured output or schemas, pass `{schema_file}`.

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
- selected harness or `--agent-command`
- high-signal findings or changed files
- whether patches were only generated or actually applied
- verification command results, if any
