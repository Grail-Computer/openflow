# Migration Workflow Template

Use this template for framework, language, dependency, API, or architecture migrations.

Planner guidance:
- Start with read-only discovery tasks before implementation tasks.
- Identify compatibility boundaries, generated files, public APIs, and risky test gaps.
- For implementation, create small write tasks that can produce independent patches.
- Prefer dependency chains over same-file parallel edits.
- Make verifier agents check behavior preservation, public API compatibility, and test relevance.
- If the migration is too broad, plan an incremental first slice rather than a sweeping rewrite.

Good task shapes:
- Inventory deprecated API usage.
- Map call sites and compatibility constraints.
- Implement a first isolated package or module migration.
- Verify build, typecheck, and smallest relevant tests.
- Synthesize remaining migration queue.

Final report should include:
- completed patch queue
- unapplied patches
- verification results
- commands run
- remaining migration tasks
