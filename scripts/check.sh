#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

usage() {
	echo "Usage: $0 [rust] [terminal] [evaluations] [semantic-index]" >&2
}

run_rust() {
	cd "$ROOT"
	cargo fmt --all --check
	# Process-level protocol journeys coordinate child lifecycles and bounded
	# deadlines. Serialize them so machine load cannot change their contract.
	cargo test --workspace --all-targets -- --test-threads=1
	cargo clippy --workspace --all-targets -- -D warnings
	cargo build --workspace --release --locked
}

run_terminal() {
	cd "$ROOT"
	node --test scripts/test/*.test.mjs
}

run_evaluations() {
	cd "$ROOT/evaluations"
	uv sync --locked
	DEEPEVAL_DISABLE_DOTENV=1 uv run pytest
	uv run ruff format --check .
	uv run ruff check .
}

run_semantic_index() {
	cd "$ROOT/services/semantic-index"
	uv sync --locked
	uv run pytest
	uv run ruff format --check .
	uv run ruff check .
}

if (($# == 0)); then
	set -- rust terminal evaluations semantic-index
fi

for suite in "$@"; do
	case $suite in
	rust) run_rust ;;
	terminal) run_terminal ;;
	evaluations) run_evaluations ;;
	semantic-index) run_semantic_index ;;
	*)
		usage
		echo "Unknown verification suite: $suite" >&2
		exit 2
		;;
	esac
done
