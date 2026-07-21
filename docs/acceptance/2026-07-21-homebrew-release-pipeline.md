# Homebrew release pipeline checkpoint

Date: 2026-07-21

## User outcome

After a future accepted semantic release, a macOS developer installs the
complete Lantern workbench with `brew install e7nt/tap/lantern` and receives a
new checksum-pinned version through `brew upgrade lantern`.

No package has been published at this checkpoint. The source repository remains
private, and the release workflow explicitly rejects that state before building
or writing to the public tap.

## Implemented boundary

- Annotated `vMAJOR.MINOR.PATCH` tags are the only release trigger.
- A manual workflow run may build private validation artifacts, but its publish
  job is structurally disabled.
- `VERSION` and every Rust package must match the tag.
- GitHub-hosted Apple Silicon and Intel runners build the pinned Helix patch set
  and locked Lantern workspace.
- Each archive includes the complete UI/runtime boundary, locked Pi package,
  semantic worker dependencies and model, exact license, and version metadata.
- SHA-256 files and GitHub provenance attestations accompany both archives.
- A narrowly scoped `HOMEBREW_TAP_TOKEN` updates only the formula in the
  separate public tap; no credential appears in a command argument or artifact.
- The formula chooses an architecture explicitly and verifies the corresponding
  checksum before installation.

Pi 0.80.6 currently bundles vulnerable copies of `brace-expansion` and
`protobufjs`. The release packager removes only those nested copies after the
locked install and verifies that Node resolves the explicitly locked patched
versions (`5.0.7` and `7.6.5`) from Lantern's package root. A future Pi upgrade
should remove this narrow packaging correction after its own dependency graph
is clean and the driver contract is revalidated.

## Verification at this checkpoint

- Release workflow structure and immutable action pins are deterministic tests.
- Formula rendering rejects invalid versions, URLs, and checksums.
- The rendered formula records AGPL-3.0-only and separate architecture assets.
- The complete canonical local gate passes.
- One hundred consecutive serialized daemon protocol-suite runs pass after the
  lifecycle fixes discovered by the first clean GitHub runs.

## Required evidence before the first tag

- Complete the remaining public-release gate and make `e7nt/lantern` public.
- Add the narrowly scoped tap token as an Actions secret.
- Install and launch the generated package on fresh supported Apple Silicon and
  Intel Macs.
- Exercise an actual install, later formula update, and `brew upgrade` without
  preserving state in the Cellar.
- Run and record the release-candidate live model evaluations.
