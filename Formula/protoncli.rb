class Protoncli < Formula
  desc "Production-ready CLI email client for ProtonMail Bridge"
  homepage "https://github.com/kvnwdev/protoncli"
  url "https://github.com/kvnwdev/protoncli/archive/refs/tags/v0.4.4.tar.gz"
  sha256 "8c45a8c6700d0811e07da0bef214f88ca39456bdcf12da7212c0061e480e853f"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "protoncli", shell_output("#{bin}/protoncli --version")
  end
end
