---
name: code-review
description: Review nt code for correctness, Rust ownership, API design, error handling, tests, simplicity, dependencies, and project constraints.
---

# Code Review Skill

Review `nt` changes with this procedure:

1. Read the changed files and the surrounding command flow.
2. Check correctness before style.
3. Check Rust ownership, lifetimes, borrowing, and unnecessary clones.
4. Review API design, command flow, and error handling.
5. Verify mutating commands append events.
6. Verify state remains visible as plain files.
7. Check that new behavior does not add hidden agent-only paths.
8. Check dependency additions against the minimal core rules.
9. Check test coverage and missing smoke checks.
10. Report findings first, ordered by severity, with file and line references.

Return:

1. Critical issues
2. Suggested fixes
3. Optional refactors
