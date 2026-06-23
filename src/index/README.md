# index/

On-disk JSON index: serialization shape, persistence, derived maps, and text
term indexing.

| File | Responsibility |
|---|---|
| `mod.rs` | Serialized metadata, vault state, persistence, and derived maps. |
| `terms.rs` | Tokenization, body/heading term indexing, and term-match queries. |
