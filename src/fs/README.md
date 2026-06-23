# fs/

Filesystem primitives: path resolution, atomic writes, and the mutation lock.

| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports the public API. |
| `paths.rs` | Home and nt-home resolution, index path, and cwd-relative paths. |
| `atomic.rs` | Atomic temp-file-and-rename writes and exclusive file creation. |
| `lock.rs` | PID-stamped index mutation lock with dead-holder recovery. |
