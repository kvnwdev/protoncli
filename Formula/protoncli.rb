class Protoncli < Formula
  desc "Production-ready CLI email client for ProtonMail Bridge"
  homepage "https://github.com/kvnwdev/protoncli"
  url "https://github.com/kvnwdev/protoncli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0924e6c08bbc423498f70e6823cbbf28b72383afd0f48168f2fb0ef59f7fe97a"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "protoncli", shell_output("#{bin}/protoncli --version")
  end
end
