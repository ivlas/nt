# fs/

Filesystem primitives: path resolution and atomic file replacement.

| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports the public API. |
| `paths.rs` | Home and nt-home resolution, index path, and cwd-relative paths. |
| `atomic.rs` | Atomic temp-file-and-rename writes. |
