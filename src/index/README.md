# index/

On-disk JSON index: serialization shape and persistence of primary metadata.
No derived maps are stored; ordering, filtering, and body matching are computed
at query time.

| File | Responsibility |
|---|---|
| `mod.rs` | Serialized primary metadata, vault state, and persistence. |
