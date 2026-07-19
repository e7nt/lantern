#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
FRONTEND_DIR="$ROOT/frontend/helix"
HELIX_DIR="$ROOT/.lantern/upstream/helix"
SEMANTIC_SERVICE="$ROOT/services/semantic-index"
SEMANTIC_ENV="$ROOT/.lantern/toolchains/semantic-index"
SEMANTIC_MODEL_CACHE="$ROOT/.lantern/toolchains/semantic-models"
HELIX_REVISION=14d6bc0febed9c692048271a8ae2362ac969c6e0
HELIX_PATCHES=(
	"$FRONTEND_DIR/patches/0001-add-lantern-range-navigation.patch"
	"$FRONTEND_DIR/patches/0002-add-picker-mouse-interaction.patch"
	"$FRONTEND_DIR/patches/0003-add-bounded-call-hierarchy-export.patch"
)
HELIX_PATCHED_FILES=(
	helix-term/src/commands/typed.rs
	helix-term/src/ui/picker.rs
	helix-view/src/document.rs
)
HELIX_PATCHED_HASHES=(
	f8358b0541aebe9491321bbf45b871d8cee26e43
	3b64ad7b5c51817168bea9486ba67dba52e0ae5c
	2e2ab18588b2b643eb3318971a215235993e6762
)

verify_revision() {
	local directory=$1
	local expected=$2
	local name=$3
	if [[ ! -d $directory/.git ]]; then
		echo "$name source is missing: $directory" >&2
		exit 1
	fi
	local actual
	actual=$(git -C "$directory" rev-parse HEAD)
	if [[ $actual != "$expected" ]]; then
		echo "$name revision mismatch: expected $expected, found $actual" >&2
		exit 1
	fi
}

verify_revision "$HELIX_DIR" "$HELIX_REVISION" Helix

if git -C "$HELIX_DIR" diff --quiet; then
	for patch in "${HELIX_PATCHES[@]}"; do
		git -C "$HELIX_DIR" apply --check "$patch"
		git -C "$HELIX_DIR" apply "$patch"
	done
else
	mapfile -t changed_files < <(git -C "$HELIX_DIR" diff --name-only)
	if [[ ${changed_files[*]} != "${HELIX_PATCHED_FILES[*]}" ]]; then
		echo "Helix contains changes outside the Lantern patch set." >&2
		exit 1
	fi
	for index in "${!HELIX_PATCHED_FILES[@]}"; do
		actual_hash=$(git -C "$HELIX_DIR" hash-object "${HELIX_PATCHED_FILES[$index]}")
		if [[ $actual_hash != "${HELIX_PATCHED_HASHES[$index]}" ]]; then
			echo "Helix patch content differs at ${HELIX_PATCHED_FILES[$index]}." >&2
			exit 1
		fi
	done
fi

cargo build --release --locked --manifest-path "$HELIX_DIR/Cargo.toml"
cargo build --release --locked --manifest-path "$ROOT/Cargo.toml"

if ! command -v uv >/dev/null; then
	echo "uv is required to prepare Lantern's pinned local semantic worker." >&2
	exit 1
fi
UV_PROJECT_ENVIRONMENT="$SEMANTIC_ENV" uv sync --locked --project "$SEMANTIC_SERVICE"
env PYTHONPATH="$SEMANTIC_SERVICE" "$SEMANTIC_ENV/bin/python" \
	-m lantern_semantic_index.prepare --model-cache "$SEMANTIC_MODEL_CACHE"

printf 'Prepared Helix %s, the semantic worker, and the maintained Lantern runtime.\n' \
	"$HELIX_REVISION"
