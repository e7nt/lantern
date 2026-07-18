#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PYTHON="$ROOT/.lantern/toolchains/semantic-index/bin/python"
SERVICE="$ROOT/services/semantic-index"
STORAGE="$ROOT/.lantern/indexes/semantic"
MODEL_CACHE="$ROOT/.lantern/toolchains/semantic-models"

if [[ ! -x $PYTHON ]]; then
	echo "Lantern semantic worker environment is missing: $PYTHON" >&2
	echo "Run: $ROOT/frontend/helix/prepare.sh" >&2
	exit 1
fi

exec env PYTHONPATH="$SERVICE" "$PYTHON" -m lantern_semantic_index.worker \
	--storage "$STORAGE" \
	--model-cache "$MODEL_CACHE"
