# AGENTS.md - openflow

## Project Shape

`openflow` is a Rust CLI that orchestrates `codex exec` runs. Keep it easy to inspect, install, and fork.

## Local Norms

- Keep the CLI state format as ordinary JSON under `.openflow/runs/<run-id>/`.
- Treat read-heavy audit/review workflows as the MVP strength; keep write workflows patch-queue based.
- Prefer deterministic validation and reporting over asking an LLM to do bookkeeping.
- Do not add npm, Node, or TypeScript tooling; keep this aligned with Codex's Rust CLI ecosystem.

## Verification

- Run `cargo test` for scheduler/plan validation changes.
- Run `cargo fmt --check` for formatting checks.
- Run `cargo clippy --all-targets --all-features` before publish if dependencies are available.
- Do not run real `openflow run` as a routine test because it spends Codex tokens.
