#!/usr/bin/env bash

set -euo pipefail

VERSION=${1:-}
ARM64_URL=${2:-}
ARM64_SHA=${3:-}

if [[ ! $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
	|| [[ ! $ARM64_SHA =~ ^[0-9a-f]{64}$ ]] \
	|| [[ $ARM64_URL != https://github.com/e7nt/lantern/releases/download/* ]]; then
	echo "Usage: $0 <version> <arm64-url> <arm64-sha256>" >&2
	exit 2
fi

cat <<EOF
class Lantern < Formula
  desc "Understanding-first AI coding workbench built around Helix"
  homepage "https://github.com/e7nt/lantern"
  version "$VERSION"
  license "AGPL-3.0-only"

  url "$ARM64_URL"
  sha256 "$ARM64_SHA"

  depends_on "git"
  depends_on "node@22"
  depends_on "python@3.12"
  depends_on "tmux"
  depends_on :macos
  depends_on arch: :arm64

  def install
    libexec.install Dir["*"]
    semantic_runtime = libexec/"libexec/lantern/semantic"
    system "tar", "-C", semantic_runtime, "-cf", semantic_runtime/"vendor.tar", "vendor"
    rm_r semantic_runtime/"vendor"
    pi_runtime = libexec/"libexec/lantern/pi"
    system "tar", "-C", pi_runtime, "-cf", pi_runtime/"node_modules.tar", "node_modules"
    rm_r pi_runtime/"node_modules"
    runtime_path = [
      Formula["git"].opt_bin,
      Formula["node@22"].opt_bin,
      Formula["python@3.12"].opt_bin,
      Formula["tmux"].opt_bin,
      ENV.fetch("PATH"),
    ].join(File::PATH_SEPARATOR)
    (bin/"lantern").write_env_script libexec/"bin/lantern", PATH: runtime_path, HOMEBREW_PREFIX: HOMEBREW_PREFIX
  end

  def post_install
    semantic_runtime = libexec/"libexec/lantern/semantic"
    archive = semantic_runtime/"vendor.tar"
    if archive.exist?
      system "tar", "-C", semantic_runtime, "-xf", archive
      rm archive
    end
    pi_runtime = libexec/"libexec/lantern/pi"
    pi_archive = pi_runtime/"node_modules.tar"
    if pi_archive.exist?
      system "tar", "-C", pi_runtime, "-xf", pi_archive
      rm pi_archive
    end
  end

  test do
    assert_match "lantern #{version}", shell_output("#{bin}/lantern --version")
  end
end
EOF
