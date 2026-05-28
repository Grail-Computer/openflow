# Openflow User Experience

This is the user journey Openflow should optimize for.

## Audience

Primary users:

- Codex CLI users who want multi-stage workflows without building orchestration glue.
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

2. Confirm Codex:

   ```bash
   codex --version
   openflow --help
   ```

3. Run in a repo:

   ```bash
   openflow run "workflow: audit this repo for auth and permission bugs" --template audit --concurrency 6
   ```

4. Watch the plan summary and approve.
5. Open the generated report path.
6. Share `report.md` or paste findings into an issue.

## Daily UX

### Audit

```bash
openflow run "workflow: find real bugs in this repo. Prioritize auth, tenant isolation, and webhooks." --template audit --concurrency 8
```

Expected result:

- plan summary
- parallel worker progress
- verifier-backed report
- no file changes

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

The included Codex skill is for users who want to stay inside Codex and ask naturally:

```text
Use Openflow to run a workflow that reviews this repo for permission bugs.
```

Codex should:

1. Check `openflow --help`.
2. Select the right template.
3. Prefer `openflow plan` for write-heavy work.
4. Run or resume the workflow.
5. Summarize `report.md`.

## What Users Should See

Openflow should make the invisible orchestration visible:

- the generated plan
- task ids and dependencies
- read/write flags
- verifier count
- run id
- report path
- patch paths

The CLI should avoid pretending everything is magic. The trust comes from the visible files.

## Friction To Remove Next

- Homebrew install.
- `openflow doctor` for Codex/Rust/git checks.
- Better live progress UI.
- `openflow github-report` for issue/PR comments.
- `openflow skill install` to install the bundled Codex skill directly.
