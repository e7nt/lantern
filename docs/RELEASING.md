# Releasing Lantern

Lantern releases are deliberate maintainer checkpoints. A release packages the
complete supported macOS workbench and updates the public `e7nt/homebrew-tap`
formula; pushing ordinary commits never publishes software.

## Release boundary

Each semantic version produces separate Apple Silicon and Intel archives. An
archive contains:

- the five Lantern Rust executables;
- the pinned, patched Helix binary and runtime;
- Lantern's Helix helpers, configuration, and theme;
- the locked Pi 0.80.6 runtime;
- the semantic worker, architecture-specific Python dependencies, and prepared
  local embedding model; and
- the AGPL license and exact version metadata.

Homebrew supplies only Git, tmux, Node.js 22, and Python 3.12 as runtime
dependencies. The formula selects the matching architecture archive and checks
its SHA-256 before installation. `brew upgrade lantern` receives a new version
only after the tap formula is updated successfully.

## One-time GitHub setup

1. Complete the public-release gate in `docs/IMPLEMENTATION_PLAN.md` and make
   `e7nt/lantern` public. The release workflow intentionally refuses to publish
   assets while the source repository is private.
2. Create a fine-grained token that can write repository contents only in
   `e7nt/homebrew-tap`.
3. Store it in the Lantern repository Actions secret
   `HOMEBREW_TAP_TOKEN`. Never place it in a workflow file, command argument,
   release artifact, or local project configuration.

## Publish a version

1. Update `VERSION` and every workspace package version to the same semantic
   version.
2. Run `./scripts/check.sh` and the required release-candidate evaluations.
3. Review the complete diff and outgoing history for credentials and private
   fixtures.
4. Create and push an annotated tag from the verified commit:

   ```bash
   git tag -a v0.1.0 -m "Lantern v0.1.0"
   git push origin v0.1.0
   ```

The tag workflow validates the tag shape, annotated-tag object, repository
visibility, version agreement, and tap credential before building. It then
builds both macOS architectures from the tag, verifies checksums, publishes one
GitHub release with provenance attestations, and writes the checksum-pinned
formula to `e7nt/homebrew-tap`.

Before the repository is public, a maintainer may manually run the `Release`
workflow on `main`. That path builds and smoke-tests both packages as private,
short-lived workflow artifacts, but it cannot create a GitHub release or alter
the tap.

Release tags and attached archives are immutable product inputs. If a release
is wrong, fix it in a new patch version; do not replace a published archive or
move its version tag.

## User verification

After the first public release:

```bash
brew install e7nt/tap/lantern
lantern --version
lantern /path/to/a/git/repository
```

Maintainers must also run the `Homebrew install acceptance` workflow. It
installs the public formula and launches the shipped workbench against a fresh
Git repository on both supported macOS architectures.

After a later release:

```bash
brew update
brew upgrade lantern
lantern --version
```

The first public tag remains blocked until both architecture packages have been
installed and launched on fresh supported Macs. Record that evidence in a
dated acceptance report rather than inferring it from a successful build job.
