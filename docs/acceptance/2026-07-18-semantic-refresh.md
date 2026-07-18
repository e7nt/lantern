# Changed-file semantic refresh — 2026-07-18

Status: accepted.

Lantern now includes tracked working-tree source in semantic revision identity.
A supported uncommitted edit therefore makes the prior index stale even when
`HEAD` has not moved. The local worker detects that state on its background
monitor, exposes `building` or `stale` to concurrent questions, rebuilds an
immutable revision, and atomically publishes it. Queries never start or wait
for a refresh, and stale candidates are never returned.

The reproducible `evaluations/run_semantic_refresh.py` journey cloned pinned
p-limit revision `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551` into a disposable
directory, built the real local FastEmbed index, changed source inside one
indexed JavaScript symbol without committing, and waited for automatic
publication.

| Measurement | Result |
| --- | ---: |
| File write to ready refreshed index | 549 ms |
| Symbols re-embedded | 1 |
| Unchanged vectors reused | 16 |
| Query state after refresh | ready |
| Required result path | `index.js` |

The three-second refresh ceiling passed. The fixture remained intentionally
dirty inside its disposable directory; the pinned upstream checkout and
Lantern worktree were not modified. The timestamped raw report remains ignored
local output because it contains machine-run metadata and disposable paths are
not product artifacts.
