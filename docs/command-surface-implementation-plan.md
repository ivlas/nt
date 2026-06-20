# Command Surface Implementation Plan

This plan implements the target contract in
[cli-syntax-spec.md](cli-syntax-spec.md): consolidate read projections under
`nt list`, consolidate metadata mutations under `nt update`, and replace the
mixed read/write `nt status` command with the read-only `nt agenda` view.

The storage model remains CommonMark notes plus the visible JSON index.

## Fixed Decisions

- `nt list` prints all active-vault notes in active-recent order using the
  current summary format.
- `nt list ids`, `tags`, and `collections` are stable one-record-per-line
  vocabularies or projections, with optional tag and collection filters.
- `nt list links <id> [from|to]` preserves the current link direction behavior.
- `nt update <id> <field> <value>` changes exactly one metadata field per call.
- `kind`, `status`, `priority`, `scheduled`, and `due` use a plain value; `-`
  clears the field.
- `tag`, `collection`, `link`, and `source` require `+value` or `-value`.
- `scheduled` and `due` are optional primary metadata in `YYYY-MM-DD` form.
- Priority is optional and ordered `S`, `A`, `B`, `C`, `D`, then no priority.
- `closed` is a system-managed UTC timestamp: terminal status sets it,
  idempotent terminal updates preserve it, and reopening clears it.
- `nt agenda` includes only `kind:todo` notes with `status:open` or
  `status:waiting`.
- The default agenda sections are Overdue, Today, Upcoming, Waiting, and
  Undated, with each note appearing exactly once.
- `today`, `week`, `overdue`, `waiting`, and `undated` are positional agenda
  views.
- Legacy commands are removed after their replacements and completion are in
  place. No compatibility aliases are retained for the next release.

## Phase 1: CLI Routing And Help

Scope: `src/cli.rs`, `src/commands.rs`, `src/help.rs`, CLI parser tests.

1. Model `list` submodes: none, `ids`, `tags [tag]`, `collections [collection]`,
   and `links` with its id and optional direction.
2. Add `update <id> <field> <value>` and `agenda [view]` command variants.
3. Route new variants to focused command functions; keep legacy routing
   temporarily so behavior can migrate incrementally.
4. Update top-level and command-specific help to match the syntax spec.

Acceptance:

- Valid target forms parse.
- Missing or extra positionals fail with actionable usage text.
- `nt help`, `nt help list`, `nt help update`, and `nt help agenda` show only
  documented target syntax.

## Phase 2: List Projections

Scope: `src/commands.rs`, `src/display.rs`, storage smoke tests.

1. Move current `ids`, tags, collections, collection filtering, and links read
   behavior behind `nt list` submodes.
2. Define stable records:
   - `list ids`: id
   - `list tags`: available tag
   - `list tags <tag>`: matching note summaries
   - `list collections`: available collection
   - `list collections <collection>`: matching note summaries
   - `list links`: one related id
3. Preserve active-vault filtering and deterministic ordering.

Acceptance:

- Each projection is one record per line and works when stdout is piped.
- Empty tag or collection vocabularies print no output.
- Link direction and deduplication match current behavior.
- Focused smoke tests cover empty and populated metadata.

## Phase 3: Unified Metadata Update

Scope: `src/commands.rs`, validation helpers, index mutation tests.

1. Add a parsed field enum instead of dispatching metadata fields with ad hoc
   string comparisons throughout command code.
2. Reuse existing validators for kind, status, tags, collections, links, and
   sources, and add validators for agenda metadata.
3. Implement single-value set/clear and set-like add/remove semantics.
4. Validate all input before writing; keep temp-file-and-rename index updates.
5. Print `updated <id> <field> <value>` after success.

Acceptance:

- Repeating `+value` or `-value` is idempotent.
- Invalid fields, operators, values, ids, and link targets leave the index
  byte-for-byte unchanged.
- Updating one field preserves every other primary metadata field.
- Tests cover every user-settable field and both applicable mutation
  directions.

## Phase 4: Agenda Metadata And Querying

Scope: `src/index.rs`, `src/query.rs`, `src/export.rs`, add parsing, rebuild,
show output, and storage/query/export tests.

1. Add optional `priority`, `scheduled`, `due`, and `closed` fields to primary
   note metadata with serde defaults for old indexes.
2. Validate real calendar dates in `YYYY-MM-DD` form using standard library
   code or an existing project date helper; do not add a dependency solely for
   parsing this field.
3. Validate priority as exactly `S`, `A`, `B`, `C`, or `D`.
4. Accept priority, scheduled, and due metadata in `nt add`, `nt update`, and
   `nt find`; accept `closed:<date>` in `nt find` only.
5. On transition to `done` or `dropped`, set `closed` to current UTC if the note
   was not already in that terminal status. Preserve it for idempotent repeats.
   Clear `closed` when status becomes `open`, `waiting`, or absent.
6. Preserve agenda metadata during rebuild and include it in show/export output.
7. Add a rebuildable exact-date derived map only if query planning benefits
   from it; do not duplicate primary data without a lookup need.

Acceptance:

- Existing indexes without the new fields load unchanged.
- Invalid dates fail before mutation.
- Add, update, clear, find, show, export, and rebuild preserve all documented
  agenda metadata semantics.
- Closed timestamps use UTC, cannot be set directly, and obey idempotent status
  transition tests.

## Phase 5: Agenda

Scope: `src/commands.rs`, `src/display.rs`, agenda smoke tests.

1. Select active-vault notes with `kind:todo` and actionable status.
2. Partition notes once, in precedence order: Overdue, Today, Upcoming,
   Waiting, Undated.
3. Treat open notes with due before today as overdue. Treat open notes due today
   or scheduled on or before today as Today. Put remaining dated open notes in
   Upcoming, all waiting notes in Waiting, and remaining open notes in Undated.
4. Implement `today`, `week`, `overdue`, `waiting`, and `undated` positional
   views. `week` includes overdue items and items due or scheduled from today
   through the following six local calendar days.
5. Use due as the Overdue relevant date. For Today and Upcoming, use the
   earliest scheduled or due date that places the note in the section. Sort by
   relevant date, priority order, then active-recent order. Sort Waiting and
   Undated by priority then active-recent order.
6. Print stable `id, status, priority, scheduled, due, title` records with `-`
   for absent values.
7. Keep the command read-only and return success with no output for an empty
   agenda.

Acceptance:

- Done, dropped, non-todo, and status-less notes are excluded.
- Open and waiting notes are distinguishable in output.
- Overdue dates are not silently hidden or mutated.
- No note appears in more than one default section.
- Ordering tests cover scheduled and due dates in the past, today, future,
  equal dates, all five priorities, and absent values.

## Phase 6: Completion And Legacy Removal

Scope: `src/completion.rs`, command/help tables, docs-content tests, smoke tests.

1. Generate completions for list submodes, update fields/operators/values,
   priorities, and agenda views.
2. Change dynamic id completion from `nt ids` to `nt list ids`.
3. Derive known tag and collection values from the corresponding list
   projections without changing their stable output.
4. Remove `ids`, `tags`, `collections`, `collection`, `links`, `tag`, `untag`,
   `collect`, `uncollect`, `kind`, `status`, `link`, and `unlink` routing, help,
   and tests.
5. Remove transitional code only after all replacement tests pass.

Acceptance:

- Bash and zsh completion tests cover the new grammar.
- Legacy commands fail as unknown commands.
- `cargo fmt`, `cargo test`, and `cargo run -- help` pass.
- `rg` finds no user-facing legacy command examples outside changelog or
  migration history.

## Suggested Chat Boundaries

1. Phase 1 only.
2. Phase 2 only.
3. Phase 3 only.
4. Phase 4 only.
5. Phase 5 only.
6. Phase 6 plus final full-suite verification.

Each chat should re-read `AGENTS.md` and `docs/cli-syntax-spec.md`, inspect the
dirty worktree before editing, and avoid committing unrelated changes.
