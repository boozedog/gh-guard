class GhGuard < Formula
  desc "Security wrapper around the GitHub CLI"
  homepage "https://github.com/yourname/gh-guard"
  url "https://github.com/yourname/gh-guard/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "PLACEHOLDER"
  license "MIT"

  depends_on "rust" => :build

  # This formula installs as 'gh', shadowing the official GitHub CLI.
  # Ensure this prefix comes before the real 'gh' in your PATH.
  conflicts_with "gh"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/gh", "guard", "version"
  end
end
