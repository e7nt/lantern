from __future__ import annotations

import argparse
from pathlib import Path

from fastembed import TextEmbedding

from .index import MODEL_NAME


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-cache", required=True, type=Path)
    arguments = parser.parse_args()
    TextEmbedding(model_name=MODEL_NAME, cache_dir=str(arguments.model_cache))
    print(f"Prepared semantic model {MODEL_NAME}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
