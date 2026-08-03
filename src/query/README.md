# query

Parses deterministic, AND-combined query expressions into bound SQL. Structured
metadata and source predicates use ordinary SQL; bare, title, and body terms use
the transactional FTS5 index. Search is unranked and has no fuzzy, semantic, or
hidden retrieval.
