# AGPL licensing checkpoint

Date: 2026-07-21

## Decision

Lantern is licensed under `AGPL-3.0-only`. This keeps the project open source
while requiring distributors and operators of modified network-accessible
versions to meet the license's corresponding-source obligations. The license
does not prohibit commercial use.

The repository uses the exact GNU AGPL version 3 text in `LICENSE`. Rust and
Python package metadata use the SPDX identifier `AGPL-3.0-only`.

## Contribution boundary

Contributions are accepted under the same license as the project. Contributors
retain their copyright, and contributing does not assign copyright or grant a
separate proprietary license.

The software license does not grant a right to imply endorsement. `Lantern` is
still a working codename and has not undergone trademark clearance.

## Verification

- `LICENSE` matches the official GNU AGPL version 3 text byte for byte.
- `cargo metadata --no-deps --format-version 1` reports `AGPL-3.0-only` for
  every workspace package.
- `git diff --check` passes.
- The canonical `./scripts/check.sh` gate passes.
