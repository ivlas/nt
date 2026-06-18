# Core Readiness

`nt` 0.1.0 is usable as the initial stable core.

This means the storage model, visible index, rebuild path, search path,
metadata commands, and shell/agent interface are coherent enough for daily use.
Future work should be fixes, polish, and features layered on this core, not a
redesign of the storage/search model.

## Readiness Checklist

| Area | Status | Evidence |
|---|---|---|
| Storage model | Ready | Canonical CommonMark note files; note bodies are not stored in the visible index. |
| Vault lifecycle | Ready | `nt init`, active vault config, vault switching, and vault inspection are covered by smoke tests. |
| Capture/read/edit/delete | Ready | `nt add`, `nt show`, `nt open`, and `nt rm` are covered by smoke tests. |
| Rebuild | Ready | `nt rebuild` reconstructs active-vault metadata/body indexes, preserves primary metadata, removes stale entries, and cleans deleted links. |
| Search | Ready | `nt find` supports documented expressions, indexed body terms, candidate narrowing, active-recent ordering, and clear unknown-field failures. |
| Metadata | Ready | Tags, collections, kind, status, and links are reflected in index/search/show behavior. |
| Shell/agent interface | Ready | Commands use stable stdout/stderr behavior and avoid hidden agent-only state. |
| Release hygiene | Ready | README quickstart, changelog, release checklist, fmt/test/clippy commands, and docs-content checks are documented. |

## Known Non-goals

- no TUI in the core
- no RAG
- no embeddings
- no semantic search
- no ranking/scoring
- no daemon
- no hidden agent memory
- no app framework or workflow engine

## Current Release Boundary

The 0.1.0 boundary is:

- keep Markdown canonical
- keep `$HOME/.nt/index.json` visible and rebuildable
- keep `nt find` deterministic and index-backed where possible
- keep shell-first workflows outside the core command surface
- layer future features on top of this storage/search model
