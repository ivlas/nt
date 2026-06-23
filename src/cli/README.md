# cli/

CLI surface: clap argument schema, help text, and shell completion generation.

| File | Responsibility |
|---|---|
| `mod.rs` | Public command, subcommand, field, view, and shell enums. |
| `help.rs` | Flagless built-in help text. |
| `completion.rs` | Bash and Zsh completion script generation, including dynamic values. |
| `completion_bash.sh` | Bash dynamic-completion shell script, included via `include_str!`. |
| `completion_zsh.sh` | Zsh dynamic-completion shell script, included via `include_str!`. |
