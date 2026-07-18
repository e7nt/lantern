from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Protocol

import numpy as np
from tree_sitter_language_pack import get_parser

SCHEMA_VERSION = 1
MODEL_NAME = "BAAI/bge-small-en-v1.5"
MAX_FILE_BYTES = 256 * 1024
MAX_SYMBOL_LINES = 16
SOURCE_LANGUAGES = {
    ".go": ("go", {"function_declaration", "method_declaration", "type_declaration"}),
    ".js": ("javascript", {"class_declaration", "function_declaration", "method_definition"}),
    ".jsx": ("javascript", {"class_declaration", "function_declaration", "method_definition"}),
    ".py": ("python", {"class_definition", "function_definition"}),
    ".rs": ("rust", {"function_item", "struct_item", "trait_item"}),
    ".ts": ("typescript", {"class_declaration", "function_declaration", "method_definition"}),
    ".tsx": ("tsx", {"class_declaration", "function_declaration", "method_definition"}),
}


class Embedder(Protocol):
    def documents(self, texts: list[str]) -> np.ndarray: ...

    def query(self, text: str) -> np.ndarray: ...


@dataclass(frozen=True)
class Symbol:
    relative_path: str
    name: str
    start_line: int
    end_line: int
    text: str
    content_hash: str

    @classmethod
    def create(
        cls, relative_path: str, name: str, start_line: int, end_line: int, text: str
    ) -> Symbol:
        digest = hashlib.sha256(
            f"{relative_path}\0{name}\0{start_line}\0{text}".encode()
        ).hexdigest()
        return cls(relative_path, name, start_line, end_line, text, digest)

    def embedding_text(self) -> str:
        return f"{self.relative_path}\n{self.name}\n{self.text}"


@dataclass(frozen=True)
class Match:
    relative_path: str
    name: str
    start_line: int
    end_line: int
    score: float


@dataclass(frozen=True)
class BuildResult:
    revision: str
    symbols: int
    embedded: int
    reused: int


def git(repository: Path, *arguments: str) -> bytes:
    return subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
        timeout=30,
    ).stdout


def repository_revision(repository: Path) -> str:
    return git(repository, "rev-parse", "HEAD").decode().strip()


def tracked_paths(repository: Path) -> list[Path]:
    return [Path(item.decode()) for item in git(repository, "ls-files", "-z").split(b"\0") if item]


def _symbol_name(node, source: bytes) -> str:
    name = node.child_by_field_name("name")
    if name is None:
        return node.type
    return source[name.start_byte : name.end_byte].decode("utf-8", errors="strict")


def extract_file_symbols(repository: Path, relative_path: Path) -> list[Symbol]:
    language = SOURCE_LANGUAGES.get(relative_path.suffix)
    if language is None:
        return []
    path = repository / relative_path
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_FILE_BYTES:
        return []
    source = path.read_bytes()
    try:
        source.decode("utf-8")
    except UnicodeDecodeError:
        return []
    tree = get_parser(language[0]).parse(source)
    pending = [tree.root_node]
    symbols = []
    while pending:
        node = pending.pop()
        if node.type in language[1]:
            raw_lines = source[node.start_byte : node.end_byte].decode().splitlines()
            text = "\n".join(raw_lines[:MAX_SYMBOL_LINES]).strip()
            if text:
                symbols.append(
                    Symbol.create(
                        str(relative_path),
                        _symbol_name(node, source),
                        node.start_point[0] + 1,
                        node.end_point[0] + 1,
                        text,
                    )
                )
        pending.extend(reversed(node.named_children))
    return symbols


def extract_repository_symbols(repository: Path) -> list[Symbol]:
    repository = repository.resolve(strict=True)
    symbols = []
    for relative_path in tracked_paths(repository):
        symbols.extend(extract_file_symbols(repository, relative_path))
    return symbols


class SemanticIndex:
    def __init__(self, storage: Path, embedder: Embedder):
        self.storage = storage
        self.embedder = embedder

    def _current_revision(self) -> str | None:
        current = self.storage / "CURRENT"
        return current.read_text(encoding="utf-8").strip() if current.is_file() else None

    def _revision_dir(self, revision: str) -> Path:
        return self.storage / "revisions" / revision

    def _load(self) -> tuple[dict, np.ndarray] | None:
        revision = self._current_revision()
        if revision is None:
            return None
        revision_dir = self._revision_dir(revision)
        manifest_path = revision_dir / "manifest.json"
        vectors_path = revision_dir / "vectors.npy"
        if not manifest_path.is_file() or not vectors_path.is_file():
            return None
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if manifest.get("schema_version") != SCHEMA_VERSION:
            raise RuntimeError("semantic index schema version is unsupported; rebuild the index")
        if manifest.get("model") != MODEL_NAME:
            raise RuntimeError("semantic index model differs; rebuild the index")
        vectors = np.load(vectors_path, allow_pickle=False)
        if len(manifest["symbols"]) != len(vectors):
            raise RuntimeError("semantic index manifest and vectors differ; rebuild the index")
        return manifest, vectors

    def build(self, repository: Path) -> BuildResult:
        repository = repository.resolve(strict=True)
        revision = repository_revision(repository)
        symbols = extract_repository_symbols(repository)
        if not symbols:
            raise RuntimeError("repository contains no supported source symbols")
        prior = self._load()
        reusable = {}
        if prior is not None:
            prior_manifest, prior_vectors = prior
            reusable = {
                item["content_hash"]: prior_vectors[index]
                for index, item in enumerate(prior_manifest["symbols"])
            }
        missing = [symbol for symbol in symbols if symbol.content_hash not in reusable]
        embedded_vectors = (
            self.embedder.documents([symbol.embedding_text() for symbol in missing])
            if missing
            else np.empty((0, 0), dtype=np.float32)
        )
        created = iter(embedded_vectors)
        vectors = [
            reusable.get(symbol.content_hash, next(created))
            if symbol.content_hash not in reusable
            else reusable[symbol.content_hash]
            for symbol in symbols
        ]
        matrix = np.asarray(vectors, dtype=np.float32)
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "model": MODEL_NAME,
            "revision": revision,
            "symbols": [asdict(symbol) for symbol in symbols],
        }
        revision_dir = self._revision_dir(revision)
        revision_dir.mkdir(parents=True, exist_ok=True)
        manifest_path = revision_dir / "manifest.json"
        vectors_path = revision_dir / "vectors.npy"
        manifest_path.write_text(json.dumps(manifest, separators=(",", ":")), encoding="utf-8")
        with vectors_path.open("wb") as output:
            np.save(output, matrix, allow_pickle=False)
            output.flush()
            os.fsync(output.fileno())
        current_tmp = self.storage / "CURRENT.tmp"
        current_tmp.write_text(f"{revision}\n", encoding="utf-8")
        current_tmp.replace(self.storage / "CURRENT")
        for directory in (self.storage / "revisions").iterdir():
            if directory.is_dir() and directory.name != revision:
                shutil.rmtree(directory)
        return BuildResult(revision, len(symbols), len(missing), len(symbols) - len(missing))

    def status(self, repository: Path) -> str:
        loaded = self._load()
        if loaded is None:
            return "absent"
        manifest, _ = loaded
        return (
            "ready"
            if manifest["revision"] == repository_revision(repository.resolve(strict=True))
            else "stale"
        )

    def query(self, repository: Path, question: str, limit: int = 4) -> list[Match]:
        loaded = self._load()
        if loaded is None:
            raise RuntimeError("semantic index is unavailable; build it before querying")
        manifest, vectors = loaded
        revision = repository_revision(repository.resolve(strict=True))
        if manifest["revision"] != revision:
            raise RuntimeError("semantic index is stale; update it before querying")
        query = np.asarray(self.embedder.query(question), dtype=np.float32)
        scores = vectors @ query
        indices = np.argsort(scores)[-limit:][::-1]
        return [
            Match(
                relative_path=manifest["symbols"][index]["relative_path"],
                name=manifest["symbols"][index]["name"],
                start_line=manifest["symbols"][index]["start_line"],
                end_line=manifest["symbols"][index]["end_line"],
                score=float(scores[index]),
            )
            for index in indices
        ]
