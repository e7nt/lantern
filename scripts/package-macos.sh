#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION=${1:-}
OUTPUT=${2:-"$ROOT/dist"}

if [[ ! $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
	echo "Usage: $0 <major.minor.patch> [output-directory]" >&2
	exit 2
fi
if [[ ! -f $ROOT/VERSION ]] || [[ $(<"$ROOT/VERSION") != "$VERSION" ]]; then
	echo "Package version $VERSION does not match the repository VERSION file." >&2
	exit 1
fi

case $(uname -m) in
arm64) ARCH=arm64 ;;
x86_64) ARCH=x86_64 ;;
*)
	echo "Unsupported macOS architecture: $(uname -m)" >&2
	exit 1
	;;
esac

if [[ $(uname -s) != Darwin ]]; then
	echo "macOS packaging must run on Darwin." >&2
	exit 1
fi

for command in cargo jq npm uv tar shasum; do
	if ! command -v "$command" >/dev/null; then
		echo "Required packaging command is not installed: $command" >&2
		exit 1
	fi
done

HELIX_ROOT="$ROOT/.lantern/upstream/helix"
for path in \
	"$HELIX_ROOT/target/release/hx" \
	"$ROOT/target/release/lantern-daemon" \
	"$ROOT/target/release/lantern-git-rail" \
	"$ROOT/target/release/lantern-terminal" \
	"$ROOT/target/release/lantern-submit"; do
	if [[ ! -x $path ]]; then
		echo "Required release binary is missing: $path" >&2
		exit 1
	fi
done

PACKAGE="lantern-$VERSION-darwin-$ARCH"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/lantern-package.XXXXXXXX")
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/$PACKAGE"
INSTALL_ROOT="$STAGE/libexec/lantern"
mkdir -p "$STAGE/bin" "$INSTALL_ROOT/bin" "$INSTALL_ROOT/helix" \
	"$INSTALL_ROOT/frontend/helix" "$INSTALL_ROOT/semantic"

install -m 0755 "$ROOT/scripts/launch-lantern.sh" "$INSTALL_ROOT/scripts-launch-lantern"
install -m 0755 "$ROOT/scripts/run-semantic-worker.sh" "$INSTALL_ROOT/scripts-run-semantic-worker"
install -m 0755 "$HELIX_ROOT/target/release/hx" "$INSTALL_ROOT/helix/hx"
cp -R "$HELIX_ROOT/runtime" "$INSTALL_ROOT/helix/runtime"
cp -R "$ROOT/frontend/helix/bin" "$INSTALL_ROOT/frontend/helix/bin"
cp -R "$ROOT/frontend/helix/config" "$INSTALL_ROOT/frontend/helix/config"
install -m 0755 "$ROOT/target/release/lantern-daemon" "$INSTALL_ROOT/bin/lantern-daemon"
install -m 0755 "$ROOT/target/release/lantern-git-rail" "$INSTALL_ROOT/bin/lantern-git-rail"
install -m 0755 "$ROOT/target/release/lantern-terminal" "$INSTALL_ROOT/bin/lantern-terminal"
install -m 0755 "$ROOT/target/release/lantern-submit" "$INSTALL_ROOT/bin/lantern-submit"
install -m 0644 "$ROOT/LICENSE" "$INSTALL_ROOT/LICENSE"
mkdir -p "$INSTALL_ROOT/licenses/helix" "$INSTALL_ROOT/licenses/rust"
install -m 0644 "$HELIX_ROOT/LICENSE" "$INSTALL_ROOT/licenses/helix/LICENSE"
printf '%s\n' "$VERSION" >"$INSTALL_ROOT/VERSION"

for manifest in "$ROOT/Cargo.toml" "$HELIX_ROOT/Cargo.toml"; do
	cargo metadata --locked --format-version 1 --manifest-path "$manifest" |
		jq -r '.packages[] | [.name, .version, (.license // "UNKNOWN"), .manifest_path] | @tsv' |
		while IFS=$'\t' read -r name package_version license manifest_path; do
			package_dir=$(dirname "$manifest_path")
			destination="$INSTALL_ROOT/licenses/rust/$name-$package_version"
			mkdir -p "$destination"
			printf '%s\n' "$license" >"$destination/SPDX"
			find "$package_dir" -maxdepth 1 -type f \
				\( -iname 'license*' -o -iname 'copying*' -o -iname 'notice*' \) \
				-exec cp {} "$destination/" \;
		done
done

mkdir -p "$INSTALL_ROOT/semantic/service"
cp -R "$ROOT/services/semantic-index/lantern_semantic_index" \
	"$INSTALL_ROOT/semantic/service/lantern_semantic_index"
PYTHON=$(uv python find 3.12)
uv pip install --python "$PYTHON" --target "$INSTALL_ROOT/semantic/vendor" \
	fastembed==0.8.0 numpy==2.4.3 onnxruntime==1.23.2 \
	tree-sitter-language-pack==1.12.5
env PYTHONPATH="$INSTALL_ROOT/semantic/vendor:$INSTALL_ROOT/semantic/service" \
	"$PYTHON" -m lantern_semantic_index.prepare \
	--model-cache "$INSTALL_ROOT/semantic/models"

mkdir -p "$INSTALL_ROOT/pi"
install -m 0644 "$ROOT/packaging/pi/package.json" "$INSTALL_ROOT/pi/package.json"
install -m 0644 "$ROOT/packaging/pi/package-lock.json" "$INSTALL_ROOT/pi/package-lock.json"
npm ci --prefix "$INSTALL_ROOT/pi" --omit=dev
PI_PACKAGE="$INSTALL_ROOT/pi/node_modules/@earendil-works/pi-coding-agent"
# Pi 0.80.6 bundles two older transitive copies. Remove only those copies so
# Node resolves the locked patched versions installed at the package root.
rm -rf "$PI_PACKAGE/node_modules/brace-expansion" "$PI_PACKAGE/node_modules/protobufjs"
for dependency in brace-expansion protobufjs; do
	resolved=$(node -e "console.log(require.resolve('$dependency/package.json', { paths: ['$PI_PACKAGE'] }))")
	expected=$(realpath "$INSTALL_ROOT/pi/node_modules/$dependency/package.json")
	if [[ $(realpath "$resolved") != "$expected" ]]; then
		echo "Pi resolved $dependency outside the locked release runtime: $resolved" >&2
		exit 1
	fi
done
[[ $(node -p "require('$INSTALL_ROOT/pi/node_modules/brace-expansion/package.json').version") == 5.0.7 ]]
[[ $(node -p "require('$INSTALL_ROOT/pi/node_modules/protobufjs/package.json').version") == 7.6.5 ]]

install -m 0755 "$ROOT/packaging/macos/pi" "$INSTALL_ROOT/bin/pi"
install -m 0755 "$ROOT/packaging/macos/lantern" "$STAGE/bin/lantern"

mkdir -p "$OUTPUT"
ARCHIVE="$OUTPUT/$PACKAGE.tar.gz"
COPYFILE_DISABLE=1 tar -C "$WORK" -czf "$ARCHIVE" "$PACKAGE"
(cd "$OUTPUT" && shasum -a 256 "$(basename "$ARCHIVE")") >"$ARCHIVE.sha256"
printf '%s\n' "$ARCHIVE"
