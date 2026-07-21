#!/usr/bin/env bash

set -euo pipefail

VERSION=${1:-}
ARM64_URL=${2:-}
ARM64_SHA=${3:-}
X86_64_URL=${4:-}
X86_64_SHA=${5:-}

if [[ ! $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
	|| [[ ! $ARM64_SHA =~ ^[0-9a-f]{64}$ ]] \
	|| [[ ! $X86_64_SHA =~ ^[0-9a-f]{64}$ ]] \
	|| [[ $ARM64_URL != https://github.com/e7nt/lantern/releases/download/* ]] \
	|| [[ $X86_64_URL != https://github.com/e7nt/lantern/releases/download/* ]]; then
	echo "Usage: $0 <version> <arm64-url> <arm64-sha256> <x86_64-url> <x86_64-sha256>" >&2
	exit 2
fi

cat <<EOF
class Lantern < Formula
  desc "Understanding-first AI coding workbench built around Helix"
  homepage "https://github.com/e7nt/lantern"
  version "$VERSION"
  license "AGPL-3.0-only"

  depends_on "git"
  depends_on "node@22"
  depends_on "python@3.12"
  depends_on "tmux"
  depends_on :macos

  on_arm do
    url "$ARM64_URL"
    sha256 "$ARM64_SHA"
  end

  on_intel do
    url "$X86_64_URL"
    sha256 "$X86_64_SHA"
  end

  def install
    libexec.install Dir["*"]
    runtime_path = [
      Formula["node@22"].opt_bin,
      Formula["python@3.12"].opt_bin,
      Formula["tmux"].opt_bin,
      ENV.fetch("PATH"),
    ].join(File::PATH_SEPARATOR)
    bin.write_env_script libexec/"bin/lantern", PATH: runtime_path, HOMEBREW_PREFIX: HOMEBREW_PREFIX
  end

  test do
    assert_match "lantern #{version}", shell_output("#{bin}/lantern --version")
  end
end
EOF
