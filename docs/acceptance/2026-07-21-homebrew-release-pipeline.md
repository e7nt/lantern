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

The first Intel validation found that ONNX Runtime versions after 1.23.2 no
longer provide CPython 3.12 Intel macOS wheels. The semantic project therefore
pins 1.23.2, the newest version present in official PyPI file metadata for all
three maintained environments: Linux, Apple Silicon macOS, and Intel macOS.

## Verification at this checkpoint

- Release workflow structure and immutable action pins are deterministic tests.
- Formula rendering rejects invalid versions, URLs, and checksums.
- The rendered formula records AGPL-3.0-only and separate architecture assets.
- The complete canonical local gate passes.
- One hundred consecutive serialized daemon protocol-suite runs pass after the
  lifecycle fixes discovered by the first clean GitHub runs.
- Private workflow run
  [`29847615493`](https://github.com/e7nt/lantern/actions/runs/29847615493)
  packages and smoke-tests both architectures successfully: Apple Silicon in
  10m25s and Intel in 12m53s. Its publish job is skipped by design.
- The downloaded 482,048,273-byte arm64 and 484,980,447-byte x86_64 archives
  match their generated SHA-256 digests and contain one architecture-specific
  top-level directory each.
- The first public install acceptance exposed Homebrew attempting to rewrite a
  vendored Python extension with an intentionally compact Mach-O header. The
  formula now keeps the native semantic vendor tree archived during Homebrew's
  linkage pass and extracts it in `post_install`, preserving the released
  runtime without disabling linkage checks for the rest of the keg.
- The same acceptance exposed Pi's prebuilt clipboard module to Homebrew's
  ad-hoc signing pass on Intel and caught a wrapper destination typo. The
  formula now preserves Pi's locked `node_modules` tree through `post_install`
  and writes the launcher explicitly to `bin/lantern`.
- The first successful installed launch then exposed that the wrapper omitted
  its declared Homebrew Git dependency from the sanitized runtime `PATH`. The
  formula now provides Git's stable `opt_bin` alongside its other dependencies.
- GitHub's Intel runner preloads python.org 3.12 symlinks into Homebrew's
  `/usr/local/bin`, which is not a clean Homebrew prefix and prevents the
  declared Python dependency from linking. Acceptance removes only symlinks
  targeting that exact runner-managed framework before testing installation.

## Required evidence before the first tag

- Complete the remaining public-release gate and make `e7nt/lantern` public.
- Add the narrowly scoped tap token as an Actions secret.
- Install and launch the generated package on fresh supported Apple Silicon and
  Intel Macs.
- Exercise an actual install, later formula update, and `brew upgrade` without
  preserving state in the Cellar.
- Run and record the release-candidate live model evaluations.
