# Openflow User Experience

This is the user journey Openflow should optimize for.

## Audience

Primary users:

- CLI agent users who want multi-stage workflows without building orchestration glue.
- Engineers running repo audits, migrations, reviews, and bug hunts.
- AI tooling people who want a transparent implementation of dynamic workflows.

Secondary users:

- Teams who want reusable `.openflow/workflows/*.md` templates.
- Maintainers who want GitHub Action reports later.

## First-Time Flow

Goal: a user gets a useful report in under five minutes.

1. Install:

   ```bash
   cargo install --git https://github.com/Grail-Computer/openflow openflow
   ```

2. Confirm Openflow and a harness:

   ```bash
   openflow doctor
   ```

   Codex is the default preset. For another harness, run doctor with the custom command template:

   ```bash
   openflow doctor --agent kimi-k2 --agent-command 'kimi-k2-cli run --prompt-file {prompt_file} --output {output_file}'
   ```

3. Run in a repo:

   ```bash
   openflow run "workflow: audit this repo for auth and permission bugs" --template audit --concurrency 6
   ```

4. Watch the plan summary and approve.
5. Open the generated report path.
6. Run `openflow validate` if the report will be shared.
7. Share `report.md` or paste findings into an issue.

## Daily UX

### Loop Model

Openflow should make agent looping concrete:

- Closed loop by default: clear goal, planned steps, deterministic observations, verifier gates, stop/handoff conditions.
- Open loop when requested: broader discovery space, but still bounded by budget, risk, status, and verification.
- Orchestrator/specialist shape: planner decomposes, workers own narrow tasks, verifiers gate acceptance, report preserves decisions.
- Reversible-only writes: workers produce patch queues from isolated worktrees; `openflow apply` is a separate command.
- Fail-closed verification: unsupported, stale, risky, or out-of-scope outputs should be rejected.
- External brake: `--brake-file <path>` blocks before the next task batch when the file contains non-whitespace text.

### Audit

```bash
openflow run "workflow: find real bugs in this repo. Prioritize auth, tenant isolation, and webhooks." --template audit --concurrency 8
```

Expected result:

- plan summary
- parallel worker progress
- verifier-backed report
- `openflow validate` confirms the run artifacts are complete
- no file changes

### Custom Harness

```bash
openflow run "workflow: audit this repo for auth bugs" \
  --template audit \
  --agent kimi-k2 \
  --agent-command 'kimi-k2-cli run --prompt-file {prompt_file} --output {output_file}' \
  --concurrency 6
```

Expected result:

- same Openflow planner/verifier UX
- workers launched through the selected harness
- run state still under `.openflow/runs/<run-id>/`

### Deeper Editing

The default UX should stay one command. Deeper customization should happen by editing the generated plan:

```bash
openflow plan "workflow: audit this repo with a stronger model for security-critical steps"
$EDITOR .openflow/runs/<run-id>/plan.json
openflow approve <run-id>
openflow resume <run-id>
```

Users can set workflow defaults once:

```json
{
  "defaults": {
    "model": "fast-default-model",
    "verifierModel": "strong-verifier-model",
    "sandbox": "read-only",
    "writeSandbox": "workspace-write"
  }
}
```

Then override only the special steps:

```json
{
  "id": "audit-payment-permissions",
  "role": "security-reviewer",
  "model": "strong-reasoning-model",
  "verifiersPerTask": 2,
  "verifierModel": "strong-verifier-model",
  "maxRetries": 2
}
```

This is the intended UX principle:

- Simple by default.
- Fully editable when users want control.
- Same state files and reports either way.

### Run Validation

```bash
openflow validate [run-id]
```

Expected result:

- state file parses
- plan still validates
- task results exist for completed tasks
- accepted verifier results are present when verification was required
- patch files and worker logs exist when referenced
- final report exists for completed, failed, or blocked runs

### Local Codex Control Loop

Openflow should support a lightweight controller loop around local Codex:

```bash
mkdir -p .codex-loop
./scripts/check.sh > .codex-loop/status.md 2>&1
openflow run "workflow: fix the failing check with the smallest safe change" \
  --template migration \
  --status-file .codex-loop/status.md \
  --brake-file .codex-loop/brake
```

Expected result:

- the status file is copied into `.openflow/runs/<run-id>/state.json`
- planner, worker, and verifier prompts include the same controller observations
- `report.md` includes a Controller Observations section
- `openflow validate` reports captured observations
- rerunning `openflow resume --status-file .codex-loop/status.md` refreshes the status object before retrying failed or blocked tasks
- creating `.codex-loop/brake` with non-empty content blocks before the next task batch

### Migration

```bash
openflow plan "workflow: migrate this app from Next 14 to Next 15" --template migration
openflow approve
openflow resume
openflow report --print
```

Expected result:

- staged plan before work starts
- write tasks isolated in worktrees
- patch files under `.openflow/runs/<run-id>/patches`
- explicit `openflow apply` step

### PR Review

```bash
openflow run "workflow: review this branch against main for correctness and security bugs" --template pr-review --concurrency 4
```

Expected result:

- severity-ordered findings
- rejected low-confidence issues
- concrete file references

## Skill UX

The included skill is for users who want to stay inside Codex and ask naturally:

```text
Use Openflow to run a workflow that reviews this repo for permission bugs.
```

Codex should:

1. Check `openflow --help`.
2. Run `openflow doctor` for a fresh install, skill install, or harness setup check.
3. Identify whether to use the default Codex preset or a user-provided harness command.
4. Select the right template.
5. Prefer `openflow plan` for write-heavy work.
6. Run or resume the workflow.
7. Run `openflow validate` before presenting a shareable or high-stakes result.
8. Summarize `report.md`.

## What Users Should See

Openflow should make the invisible orchestration visible:

- the generated plan
- task ids and dependencies
- read/write flags
- verifier count
- model and harness overrides
- run id
- report path
- validation status
- patch paths

The CLI should avoid pretending everything is magic. The trust comes from the visible files.

## Friction To Remove Next

- Homebrew install.
- Better live progress UI.
- `openflow github-report` for issue/PR comments.
