#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PYTHON=${LANTERN_SEMANTIC_PYTHON:-"$ROOT/.lantern/toolchains/semantic-index/bin/python"}
SERVICE=${LANTERN_SEMANTIC_SERVICE:-"$ROOT/services/semantic-index"}
STORAGE=${LANTERN_SEMANTIC_STORAGE:-"$ROOT/.lantern/indexes/semantic"}
MODEL_CACHE=${LANTERN_SEMANTIC_MODEL_CACHE:-"$ROOT/.lantern/toolchains/semantic-models"}
PYTHON_VENDOR=${LANTERN_SEMANTIC_VENDOR:-}

if [[ ! -x $PYTHON ]]; then
	echo "Lantern semantic worker environment is missing: $PYTHON" >&2
	echo "Run: $ROOT/frontend/helix/prepare.sh" >&2
	exit 1
fi

python_path=$SERVICE
if [[ -n $PYTHON_VENDOR ]]; then
	python_path="$PYTHON_VENDOR:$python_path"
fi

exec env PYTHONPATH="$python_path" "$PYTHON" -m lantern_semantic_index.worker \
	--storage "$STORAGE" \
	--model-cache "$MODEL_CACHE"
