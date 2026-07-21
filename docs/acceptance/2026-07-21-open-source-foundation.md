# Open-source foundation acceptance — 2026-07-21

## Outcome

Lantern now has one contributor path instead of a collection of commands that
assumed a prepared maintainer workspace:

- `frontend/helix/prepare.sh` fetches the exact recorded Helix commit into a
  clean checkout, rejects remote/revision drift, applies the audited patches,
  and builds locked runtime dependencies;
- `scripts/check.sh` is the canonical local and CI verification entry point;
- Rust 1.96.1, Node.js 22, and Python 3.12 are explicitly pinned;
- CI runs deterministic Rust, terminal, DeepEval, and semantic contracts with
  read-only repository permissions and commit-pinned external actions;
- `CONTRIBUTING.md` states supported environments, setup, review evidence, and
  safe reporting expectations;
- `SECURITY.md` provides private vulnerability reporting and describes the
  trusted-workbench boundary without promising an unsupported SLA.

The workflow does not use provider credentials, `pull_request_target`, mutable
action tags, deployment permissions, or silent platform fallbacks.

## Verification

- `./scripts/check.sh`: passed.
- Rust workspace: 117 tests passed; formatting, Clippy with warnings denied,
  and the locked release build passed.
- Terminal and open-source foundation contracts: 16 tests passed.
- DeepEval: 55 tests passed; evaluation Ruff checks passed.
- Semantic index: 5 tests passed; service Ruff checks passed.
- Shell syntax validation passed for preparation, verification, and launch
  scripts.

The first full gate attempt exposed one transient daemon journey failure; the
focused rerun passed and the second complete canonical run passed. This is
recorded rather than presented as two clean repetitions. Fresh-machine CI will
provide independent clean-checkout evidence.

## License decision

The maintainer subsequently selected AGPL-3.0-only. The repository contains the
unaltered GNU license text, package metadata uses the SPDX identifier, and the
contribution guide applies the same inbound terms without copyright assignment.

## Remaining release blockers

- The complete preparation and launch journey has not run on a fresh supported
  Linux machine or macOS.
- Dependency license, vulnerability, and secret scanning are not yet CI gates.
- Lantern has no versioned release artifact, checksum, installer, upgrade, or
  rollback contract.
