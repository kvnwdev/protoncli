class Protoncli < Formula
  desc "Production-ready CLI email client for ProtonMail Bridge"
  homepage "https://github.com/kvnwdev/protoncli"
  url "https://github.com/kvnwdev/protoncli/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "8e8052a0c2d1b3568998cc84bea4b37c23a7db71d547cf8f34b6b109f2f55a30"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "protoncli", shell_output("#{bin}/protoncli --version")
  end
end
