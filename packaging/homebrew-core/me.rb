class Me < Formula
  desc "Local meaning environment"
  homepage "https://github.com/inshell-art/me"
  url "https://github.com/inshell-art/me/archive/refs/tags/v0.8.2.tar.gz"
  sha256 "<release-sha256>"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/me-cli")
  end

  test do
    assert_match "ME ", shell_output("#{bin}/me --version")
    workspace = testpath/"ME"
    system bin/"me", "new", workspace
    system bin/"me", "--workspace", workspace, "fsck"
    output = shell_output("#{bin}/me --workspace #{workspace} welcome --json")
    assert_match "me.welcome", output
  end
end
