# AGENTS.md - openflow

## Project Shape

`openflow` is a Rust CLI that orchestrates non-interactive CLI agent harnesses. Codex is the default preset, but the project should stay harness-independent.

## Local Norms

- Keep the CLI state format as ordinary JSON under `.openflow/runs/<run-id>/`.
- Treat read-heavy audit/review workflows as the MVP strength; keep write workflows patch-queue based.
- Prefer deterministic validation and reporting over asking an LLM to do bookkeeping.
- Do not add npm, Node, or TypeScript tooling; keep the CLI Rust-native.
- Keep Codex as a first-class preset, not a hard product dependency. Custom harnesses should work through `--agent-command`.
- Keep structured-output schemas strict-compatible: list every object property in `required` when `additionalProperties: false`, and model optional overrides as nullable values.
- Keep `openflow install-skill` and files under `skills/` in sync; the installer embeds skill files at compile time.

## Verification

- Do not add GitHub Actions or other automatic remote CI unless the user explicitly asks for it.
- Codex must run `./scripts/check.sh` locally before publishing changes.
- Run `cargo test` for scheduler/plan validation changes.
- Run `cargo fmt --check` for formatting checks.
- Run `cargo clippy --all-targets --all-features` before publish if dependencies are available.
- Run `openflow doctor` after changing bundled skills, skill installation, or harness setup checks.
- Run `openflow validate` in a disposable run after changing run-state or artifact layout behavior.
- Do not run real `openflow run` as a routine test because it spends real agent tokens or credits.
- When a live Codex-backed test is explicitly needed, run it only in a disposable repo; the Codex CLI may need sandbox escalation to write its local state.
