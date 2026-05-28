# Audit Workflow Template

Use this template for codebase-wide audits, bug hunts, security reviews, dead-code analysis, and risk scoping.

Planner guidance:
- Prefer read-only tasks.
- Split by subsystem, route, package, risk class, or file cluster.
- Every task should produce evidence-backed findings with file paths and reproduction or inspection notes.
- Avoid duplicate work by assigning non-overlapping scope.
- Require verifier agents to reject speculative findings and findings without concrete evidence.
- Set riskLevel based on potential production impact.
- Use maxConcurrency between 4 and 12 unless the requested scope is tiny.

Good task shapes:
- Explore authentication and session handling.
- Explore permission checks around write APIs.
- Explore background jobs and retry/idempotency behavior.
- Explore input validation and unsafe deserialization.
- Explore dead code or duplicated abstractions in one package.

Final report should separate:
- verified findings
- rejected or low-confidence findings
- quick wins
- deeper follow-up work
