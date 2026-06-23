# listing/

`nt list` request parsing, field projection, and table rendering.

| File | Responsibility |
|---|---|
| `mod.rs` | List request parsing, compatibility forms, and filter dispatch. |
| `field.rs` | `ListField` enum, projection parsing, and per-field rendering. |
| `render.rs` | Row and table layout for TTY and pipe output. |
