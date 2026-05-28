# PR Review Workflow Template

Use this template to review a branch, pull request diff, or uncommitted change set.

Planner guidance:
- Prefer read-only tasks.
- Split review by risk class rather than by style category.
- Include one task that maps the changed execution paths.
- Include independent tasks for correctness, security, test adequacy, and operational risk when relevant.
- Verifiers should reject style-only comments unless they hide real behavior risk.
- Require findings to include file paths, impacted behavior, and a concrete reproduction or reasoning path.

Good task shapes:
- Map changed files and runtime entry points.
- Review correctness and edge cases.
- Review security and permission boundaries.
- Review tests for meaningful coverage.
- Review deployment, migration, or config risk.

Final report should lead with actionable findings ordered by severity.
