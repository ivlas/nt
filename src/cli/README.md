# cli/

CLI surface and process-facing input, output, path, and terminal adapters.

| File | Responsibility |
|---|---|
| `mod.rs` | Public command grammar. |
| `help.rs` | Flagless built-in help text. |
| `input.rs` | Injected body input and editor process execution. |
| `paths.rs` | Canonical home-directory resolution. |
| `rendering.rs` | Stable redirected and terminal output rendering. |
| `terminal.rs` | Terminal detection and color policy. |
