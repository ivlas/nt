# nt Design

`nt` is a local, agent-first knowledge and memory layer. Its goals are fast
capture, fast deterministic retrieval, bounded context construction, durable
hierarchical organization, low LLM token cost, and full local ownership.

## Implemented Architecture

`clap` parses the flagless CLI, command handlers orchestrate operations, and
`src/repository/mod.rs` owns SQLite connection setup, schema initialization,
direct SQL persistence, and row materialization. Query evaluation remains
explicit and deterministic. `$EDITOR` integration uses temporary files, but
canonical bodies remain in SQLite.

The database is `$HOME/.nt/nt.sqlite3`. Foreign keys are enabled on every
connection and mutations use transactions. The schema contains:

- `vaults`: UUIDv7 id, unique logical name, creation time
- `collections`: UUIDv7 id, one owning vault, name, creation time
- `notes`: UUIDv7 id, home collection, CommonMark body, scalar metadata
- `note_collections`: many-to-many collection memberships
- `note_tags`, `note_sources`, and `note_links`: note-associated values and links
- `schema_version`: current schema-version gate

A deferred composite foreign key requires every `notes.home_collection_id` pair
to exist in `note_collections`. This makes home a real membership. A note has one
home but may reference collections from multiple vaults.

## Storage Decisions

SQLite replaces canonical Markdown files, configured filesystem vaults,
active-vault state, and JSON indexes. It gives body and metadata updates one
transaction boundary and removes file/index reconciliation and rebuild paths.

UUIDv7 replaces timestamp-derived `NT...` ids for notes, vaults, and
collections. Creation timestamps remain explicit fields rather than being
decoded from identifiers.

Mutations issue targeted `INSERT`, `UPDATE`, and `DELETE` statements inside
short transactions. They do not load or rewrite unrelated rows. Note creation
atomically resolves collections and inserts the note and its relationships;
updates touch only the selected scalar or set value; removal validates every id
before deleting any note.

## Retrieval Decisions

`list` produces compact metadata projections; `find` narrows candidates; `show`
retrieves one exact body. This staged interface lets agents bound context and
avoid emitting every body. Exact-note commands use id-scoped repository reads.
`list`, `find`, and `agenda` currently materialize note candidates and their
relationships in one read transaction, then filter and project in Rust. Their
compact output bounds stdout and agent context, not database reads. SQL filter
and projection pushdown remains future work.

Body search reads SQLite text and remains unranked, case-insensitive, and
AND-combined. Quoted multiword `body:` values match all terms, not an exact
phrase.

There is no scoring, vector database, embeddings, semantic search, hidden
retrieval, daemon, or RAG system. These are not required for the implemented
deterministic retrieval contract.

## Metadata Decisions

Vaults are top-level logical namespaces. Collections belong to exactly one
vault and are addressed as `<vault>/<collection>`. Collections may use `/`
inside the collection portion for durable hierarchy without creating files.

Home records canonical ownership. Additional memberships are references. Tags
remain sparse topics; links are explicit UUID relationships; sources remain
external references. Note bodies stay ordinary CommonMark without nt-specific
link syntax.

## Interface Decisions

Core workflows remain positional and compose with stdin, stdout, pipes, and
`$EDITOR`. Successful mutations print one short line. Redirected projections
are stable, tab-separated, and one record per line.

Agents and humans use the same interface. The user directs writes; there is no
agent-only command or hidden mutation path. There is no application-level
writer lock. SQLite provides process-level write serialization, rollback, and a
five-second busy timeout; one user-directed writer at a time remains the
recommended workflow.

## Decision Status

The SQLite relational model, UUIDv7 identities, logical vaults, vault-owned
collections, cross-vault memberships, and one required home collection are the
current storage contract. Markdown exports are portable snapshots, not primary
storage.

Long-term derived memory nodes and context-budget commands need a separate,
explicit behavioral design before they become public schema or CLI surface.

## Development And Release

Run before release:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets
cargo run -- help
```
