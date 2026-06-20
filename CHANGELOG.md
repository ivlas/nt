# Changelog

## Unreleased

### Changed

- `nt list` supports explicit comma-separated metadata projections and shared
  structured filters. Bare `nt list` prints every indexed metadata field.
- List rows use tab-separated columns; the existing `ids`, `titles`, `tags`,
  `collections`, and `links` forms remain available for compatibility.

## 0.1.0

Initial stable core.

### Added

- Markdown-first note capture with canonical CommonMark files.
- Active vault initialization and selection.
- Visible JSON index at `$HOME/.nt/index.json`.
- Metadata commands for tags, collections, kind, status, links, and sources.
- Rebuildable derived maps and body term index.
- Indexed `nt find` candidate narrowing.
- Deterministic active-recent output.
- Shell completion generation.
- Shell-first workflow documentation.
