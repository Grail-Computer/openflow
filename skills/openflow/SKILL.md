---
name: openflow
description: Use Openflow to plan, run, verify, resume, and report dynamic workflows with Codex or any non-interactive CLI agent harness when a user asks for a workflow, parallel agents, multi-stage audit, migration, review, or verified fan-out/fan-in task.
---

# Openflow

> **When to use:** Use this skill when the user asks for a "workflow", "dynamic workflow", "parallel agents", "fan out", "multi-agent audit", "verified review", "migration plan", or any broad task that benefits from planned stages and independent verification.

## Preconditions

1. Check that `openflow` is installed with `openflow --help`.
2. Run `openflow doctor` when checking a fresh install, skill install, or harness setup.
3. Identify the target harness. Use the default Codex preset unless the user provides another harness command.
4. For the Codex preset, check `codex --version`. For a custom harness, check the binary or command the user gave.
5. Work from the target repository root.
6. For write workflows, make sure the repo is a git repository and inspect `git status --short` before running.

## Decision Rule

Use Openflow orchestration when at least two are true:

- The task has independent research, coding, review, migration, QA, docs, or design tracks.
- The task is broad enough that an explicit success contract would reduce drift.
- The task has risk: destructive edits, external writes, deploys, secrets, production data, billing, user accounts, or large repo-wide changes.
- Verification benefits from a separate pass from implementation.
- The workflow could become a reusable recipe or project template.
- The user explicitly asks for a dynamic workflow, swarm, parallel agents, fan-out/fan-in, or Openflow.

If the task is small, do it directly and mention that full workflow orchestration was unnecessary.

## Default UX

For read-heavy audits or reviews, run a single command:

```bash
openflow run "workflow: <user request>" --template audit --concurrency 6
```

Keep this path simple. Do not add model, harness, sandbox, retry, or verifier flags unless the user asked for them or the workflow clearly requires them.

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

## Codex Goal Mode

Codex goal mode is a persistent execution mode for a concrete objective. Use it when the user asks Openflow to run a broad workflow end-to-end, or when the workflow will likely span multiple turns, retries, validation, and final reporting.

When goal mode tools are available:

1. Start a goal with the full user objective, not just the next shell command.
2. Include the target repo, selected harness, risk gates, expected report, and `openflow validate` requirement in the goal.
3. Continue until the workflow is complete and the report has been summarized, or until the goal is genuinely blocked.
4. Do not enter goal mode for a small one-shot task, a plan-only discussion, or a request that only asks for command examples.

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

## Per-Step Customization

Openflow plans support workflow defaults and task-level overrides. Use them when the user wants different models, harnesses, sandboxes, verifier counts, or retry behavior for specific steps.

Workflow-level defaults go in `defaults`:

```json
{
  "defaults": {
    "model": "gpt-5",
    "agent": "codex",
    "sandbox": "read-only",
    "writeSandbox": "workspace-write",
    "verifierModel": "gpt-5"
  }
}
```

Task-level overrides go on the task:

```json
{
  "id": "audit-auth-boundaries",
  "role": "security-reviewer",
  "model": "gpt-5-high",
  "verifiersPerTask": 2,
  "verifierModel": "gpt-5",
  "maxRetries": 2
}
```

Precedence is built-in/CLI defaults, then workflow defaults, then task overrides.

## Operating Rules

1. Prefer read-only workflows unless the user clearly wants code changes.
2. Use `openflow plan` first when a workflow includes write tasks, expensive scope, or uncertain blast radius.
3. If the user wants per-step customization, edit `.openflow/runs/<run-id>/plan.json` before approval/resume.
4. Read `openflow status` before resuming a run.
5. Use `openflow report --print` to summarize results back to the user.
6. Do not apply patches automatically unless the user asked for that. If applying, run `openflow apply` and report what changed.
7. Use `openflow validate` before presenting a shareable or high-stakes result.
8. If a run fails, inspect `.openflow/runs/<run-id>/planner.log`, task `worker.log`, and verifier logs before retrying.

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

When reporting back to the user, include:

- run id
- final status
- report path
- selected harness or `--agent-command`
- high-signal findings or changed files
- whether patches were only generated or actually applied
- validation status
- verification command results, if any
