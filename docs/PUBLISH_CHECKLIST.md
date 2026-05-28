# Publish Checklist

Use this before publishing Openflow or cutting a release.

## Checks

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
```

## Leak Scan

Before pushing a public repo, run:

```bash
find . -name .env -o -name '*.pem' -o -name '*.key'
rg -n "OPENAI_API_KEY|ANTHROPIC_API_KEY|GITHUB_TOKEN|DATABASE_URL|PRIVATE_KEY|BEGIN RSA|BEGIN OPENSSH|ghp_|sk-[A-Za-z0-9]|xox[baprs]-|AKIA[0-9A-Z]{16}" .
```

Expected result: no real secrets. Documentation may mention placeholder variable names, but should not contain values.

## Public Repo Settings

- Repository: `Grail-Computer/openflow`
- Visibility: public
- Default branch: `main`
- Description: `Open-source dynamic workflow orchestration for Codex CLI`
- Topics: `codex`, `agents`, `workflow`, `rust`, `cli`

## Release Assets Later

- Homebrew tap
- prebuilt macOS/Linux binaries
- GitHub Action
- demo GIF
