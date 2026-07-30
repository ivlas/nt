# nt Design

`nt` is a local, agent-first knowledge and memory layer. Its goals are fast
capture, fast deterministic retrieval, bounded context construction, durable
hierarchical organization, low LLM token cost, and full local ownership.

## Implemented Architecture

`clap` parses the flagless CLI, command handlers orchestrate operations, and
`src/index/mod.rs` owns SQLite schema and persistence. Query evaluation remains
explicit and deterministic. `$EDITOR` integration uses temporary files, but
canonical bodies never leave SQLite.

The database is `$HOME/.nt/nt.sqlite3`. Foreign keys are enabled on every
connection and mutations use transactions. The schema contains:

- `vaults`: UUIDv7 id, unique logical name, creation time
- `collections`: UUIDv7 id, one owning vault, name, creation time
- `notes`: UUIDv7 id, home collection, CommonMark body, scalar metadata
- `note_collections`: many-to-many collection memberships
- normalized tag, source, and note-link tables

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

The current repository layer materializes primary rows for each command and
writes validated state transactionally. This preserves simple deterministic
query behavior while establishing the relational model. Future scale work can
move projections and filters into SQL without changing storage semantics.

## Retrieval Decisions

`list` produces compact metadata projections; `find` narrows candidates; `show`
retrieves one exact body. This staged interface lets agents bound context and
avoid loading every note. Body search reads SQLite text and remains unranked,
case-insensitive, and AND-combined. Quoted multiword `body:` values match all
terms, not an exact phrase.

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
agent-only command or hidden mutation path. The supported mutation workflow is
one user-directed writer at a time, with SQLite providing process-level write
serialization and rollback.

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
