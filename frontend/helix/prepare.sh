#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
FRONTEND_DIR="$ROOT/frontend/helix"
HELIX_DIR="$ROOT/.lantern/upstream/helix"
LAZYGIT_DIR="$ROOT/.lantern/upstream/lazygit"
LAZYGIT_BIN="$ROOT/.lantern/toolchains/lazygit/lazygit"
HELIX_REVISION=14d6bc0febed9c692048271a8ae2362ac969c6e0
LAZYGIT_REVISION=080da5cacfcff63a89ea23493bb91b11b0612876
HELIX_PATCHES=(
	"$FRONTEND_DIR/patches/0001-add-lantern-range-navigation.patch"
	"$FRONTEND_DIR/patches/0002-add-picker-mouse-interaction.patch"
)
HELIX_PATCHED_FILES=(
	helix-term/src/commands/typed.rs
	helix-term/src/ui/picker.rs
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
verify_revision "$LAZYGIT_DIR" "$LAZYGIT_REVISION" Lazygit

if git -C "$HELIX_DIR" diff --quiet; then
	for patch in "${HELIX_PATCHES[@]}"; do
		git -C "$HELIX_DIR" apply --check "$patch"
		git -C "$HELIX_DIR" apply "$patch"
	done
elif ! cmp -s \
	<(git -C "$HELIX_DIR" diff -- "${HELIX_PATCHED_FILES[@]}") \
	<(cat "${HELIX_PATCHES[@]}"); then
	echo "Helix contains changes other than the exact Lantern patch set." >&2
	echo "Use a clean checkout at $HELIX_REVISION before preparing Lantern." >&2
	exit 1
fi

cargo build --release --locked --manifest-path "$HELIX_DIR/Cargo.toml"
cargo build --release --locked --manifest-path "$ROOT/Cargo.toml"

if ! command -v go >/dev/null; then
	echo "Go is required to build the pinned Lazygit source." >&2
	exit 1
fi
mkdir -p "$(dirname "$LAZYGIT_BIN")"
(cd "$LAZYGIT_DIR" && go build -trimpath -o "$LAZYGIT_BIN" ./)

printf 'Prepared Helix %s, Lazygit %s, and the maintained Lantern runtime.\n' \
	"$HELIX_REVISION" "$LAZYGIT_REVISION"
