# Shell-first Workflows

`nt` should produce simple deterministic output. Shell tools provide paging,
fuzzy selection, preview, and batching outside the core command surface.

The preferred model is:

```text
nt find / nt show / nt edit
+ less / fzf / awk / xargs
```

This keeps `nt` predictable for humans and agents. A TUI is intentionally
deferred and is not part of the current core.

## Search and Inspect

```sh
nt find rust ownership
nt show NT20260616T101500
nt show NT20260616T101500 | less
```

## Page Search Results

```sh
nt find rust | less
```

## Fuzzy Select Then Show

```sh
nt find rust | fzf | awk '{print $1}' | xargs nt show
```

## Fuzzy Select With Preview

```sh
nt find rust | fzf --preview 'nt show {1}'
```

## Fuzzy Select Then Edit

```sh
nt find rust | fzf --preview 'nt show {1}' | awk '{print $1}' | xargs nt edit
```

## Extract Ids

```sh
nt find tag:rust | awk '{print $1}'
```

## Batch Inspect Exact Ids

```sh
nt ids | fzf --multi | xargs -n1 nt show
```
