# commands/

Application layer: command dispatch, per-command handlers, and shared helpers.

`mod.rs` routes `Command` variants to handler functions and owns the shared
validators, status-transition logic, and index-access helpers used across
handlers.

| File | Responsibility |
|---|---|
| `mod.rs` | Command routing, shared validators, status transitions, and index helpers. |
| `init.rs` | Logical vault and inbox creation. |
| `add.rs` | `note`/`todo`, creation metadata parsing, and editor plumbing. |
| `show.rs` | `show`, `open`, and `find`. |
| `rm.rs` | Transactional note and relationship removal. |
| `update.rs` | `update` and the update operation model. |
| `list.rs` | `list` orchestration and link graph rendering. |
| `agenda.rs` | `agenda` sections, selection, and ordering. |
| `export_cmd.rs` | Portable Markdown snapshot export. |
| `config.rs` | Database-path inspection and logical vault listing. |
