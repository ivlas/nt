# nt Architecture

`nt` is a modular monolith with application, domain, and shared-core layers.
It remains one Cargo package and uses explicit compile-time composition rather
than plugins, registries, or generic repository traits.

## Layers

The application layer owns the CLI, process adapters, command orchestration,
and the canonical database manifest:

```text
src/cli/
src/commands/
src/app.rs
src/schema.rs
src/lib.rs
src/main.rs
```

`src/lib.rs` is a thin binary-support boundary. `run_process` is the only
intentional public Rust entry point; domain, repository, CLI grammar, and schema
APIs are implementation details rather than a supported SDK.

Domains own their values, query model, persistence operations, and schema
fragments. The current domain is notes:

```text
src/domains/note/
```

Shared core owns infrastructure that has no note semantics:

```text
src/core/storage/
```

`src/error.rs` is shared crate infrastructure. It defines the concrete error
vocabulary used across the application, domains, and shared core, including the
stable process categories consumed by the binary adapter.

The storage core opens and configures SQLite connections, publishes new
databases atomically, and initializes and validates an explicitly supplied
schema manifest. It does not contain note types, table names, or FTS names.

## Dependency Direction

```text
application -> domains -> core
application -----------> core
all layers ------------> shared errors
```

The production dependency rules are:

```text
core does not import a domain
note does not import CLI, commands, App, or another domain
commands may orchestrate domain operations
schema.rs may compose domain schema fragments
```

Tests may construct the complete application across these boundaries to verify
integration behavior.

## Schema Composition

`src/schema.rs` is the closed application schema manifest. It owns the
application ID and schema version and explicitly composes required domain
fragments. `src/domains/note/schema.rs` owns all note tables, indexes,
triggers, FTS definitions, and derived FTS object requirements.

`src/core/storage/schema_engine.rs` knows only schema descriptors and mechanics:
transactional initialization, identity inspection, exact object validation,
version checks, and unknown-trigger rejection. A domain is never registered
dynamically or made optional at runtime.

Version-1 SQL definitions are a compatibility boundary. Structural refactors
must leave their stored `sqlite_schema.sql` values and creation order unchanged.
The independent fixture in `tests/fixtures/v1_schema.sql` protects that contract
through the compiled binary interface.

## Extension

A future domain adds its own model, query model, repository, and schema fragment.
Application commands orchestrate it, and the application manifest explicitly
composes its schema if it shares the canonical database. New abstractions should
be introduced only when concrete domain use demonstrates that they are shared.

Append-only memory still requires its own workflow-backed RFC. This architecture
does not decide memory behavior or storage merely by reserving a directory.
