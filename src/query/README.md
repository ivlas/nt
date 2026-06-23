# query/

`nt find` / `nt list` query language: parse, plan, evaluate.

`mod.rs` exposes `Query` and `QueryExpr`; the sub-modules split the parse,
candidate-planning, and evaluation phases.

| File | Responsibility |
|---|---|
| `mod.rs` | `Query` and `QueryExpr` types, public parse/match API. |
| `parse.rs` | Expression parsing, field validators, and unknown-field suggestions. |
| `eval.rs` | Metadata and body match verification. |
| `plan.rs` | Candidate-set algebra and index lookups for query planning. |
| `suggest.rs` | Edit-distance field suggestion utility. |
