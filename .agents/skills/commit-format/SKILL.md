---
name: commit-format
description: >-
  Group nt workspace changes into focused commits and suggest concise commit
  messages using the project's prefixes: fix, refactor, chore, docs, and test.
  Use when the user asks what to commit, how to split commits, how to write
  commit messages, or asks for commit grouping guidance.
---

# Commit Format Skill

Use this procedure when grouping or preparing `nt` commits:

1. Read `git status --short`.
2. Inspect tracked diffs with `git diff --stat` and file-specific diffs when needed.
3. Include untracked files in the grouping; `git diff` does not show them.
4. Group files by one primary reason for change.
5. Use one of these commit prefixes:
   - `fix: ...`
   - `refactor: ...`
   - `chore: ...`
   - `docs: ...`
   - `test: ...`
6. Keep commit messages concise and imperative.
7. Do not mix docs-only changes with behavior changes unless they are tightly coupled.
8. Do not suggest committing unrelated dirty files with the current task.

Prefer these meanings:

- `fix`: correct broken behavior.
- `refactor`: restructure code without changing behavior.
- `chore`: scaffolding, dependency, build, maintenance, or initial implementation work.
- `docs`: documentation-only changes.
- `test`: test-only changes.

Return commit suggestions as:

1. Commit message
2. Files to include
3. Short reason for grouping
