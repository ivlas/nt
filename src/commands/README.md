# commands/

Application layer: command dispatch, per-command handlers, and shared helpers.

`mod.rs` routes `Command` variants to handler functions and owns the shared
validators, status-transition logic, and index-access helpers used across
handlers.

| File | Responsibility |
|---|---|
| `mod.rs` | Command routing, shared validators, status transitions, and index helpers. |
| `init.rs` | `init` and Markdown import for existing flat vaults. |
| `add.rs` | `note`/`todo`, creation metadata parsing, and editor plumbing. |
| `show.rs` | `show`, `open`, and `find`. |
| `rm.rs` | `rm` and index removal. |
| `update.rs` | `update` and the update operation model. |
| `list.rs` | `list` orchestration and link graph rendering. |
| `agenda.rs` | `agenda` sections, selection, and ordering. |
| `export_cmd.rs` | `export` and active-vault guards. |
| `config.rs` | `config show` and `config vault`. |
