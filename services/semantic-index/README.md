# Lantern semantic index worker

This private local worker owns disposable semantic-index artifacts. It is not
an answer engine and never changes repository source.

The worker:

- extracts bounded symbols from tracked Python, JavaScript, TypeScript, Go, and
  Rust source using pinned Tree-sitter grammars;
- embeds at most the first 16 lines of each symbol with the pinned local model;
- stores immutable revision directories selected by one atomic `CURRENT`
  pointer;
- reuses vectors by content hash and embeds only changed symbols;
- refuses stale indexes instead of returning old candidates;
- returns repository-relative symbol locations, which the daemon must verify
  against current source before supplying them to Pi.

## Development

```bash
uv sync --locked
uv run pytest
uv run ruff format --check .
uv run ruff check .
```

The JSONL worker protocol is version 1. `open_workbench` reports `ready` or
`building`; background completion emits `index_ready` or `index_failed`.
Queries made before readiness return their explicit state and no matches.
Downloaded model data, virtual environments, and built indexes belong under
`.lantern/` and must never be committed.
