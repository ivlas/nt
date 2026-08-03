# listing/

`nt list` request parsing, field projection, and table rendering.

| File | Responsibility |
|---|---|
| `mod.rs` | Projection and structured-filter request parsing. |
| `field.rs` | `ListField` enum, projection parsing, and per-field rendering. |
| `render.rs` | Row and table layout for TTY and pipe output. |
