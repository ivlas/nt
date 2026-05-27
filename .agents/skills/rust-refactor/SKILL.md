---
name: rust-refactor
description: Refactor Rust code while preserving behavior and keeping dependencies minimal.
---

# Rust Refactor Skill

Refactor Rust code with this procedure:

1. Identify the behavior that must be preserved.
2. Keep the public command surface unchanged unless explicitly requested.
3. Prefer small functions and explicit control flow.
4. Use standard library types before adding dependencies.
5. Keep errors plain and actionable.
6. Avoid broad rewrites when a local change is enough.
7. Run `cargo fmt` and `cargo test` after changes.
