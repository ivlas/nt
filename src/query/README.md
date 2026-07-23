# query/

`nt find` / `nt list` query language: parse, evaluate.

`mod.rs` exposes `Query` and `QueryExpr`; the sub-modules split parsing,
evaluation, and field suggestions. Matching evaluates each note's primary
metadata directly and reads Markdown bodies from disk for body terms.

| File | Responsibility |
|---|---|
| `mod.rs` | `Query` and `QueryExpr` types, public parse/match API. |
| `parse.rs` | Expression parsing, field validators, and unknown-field suggestions. |
| `eval.rs` | Metadata matching and on-demand body reads. |
| `suggest.rs` | Edit-distance field suggestion utility. |
