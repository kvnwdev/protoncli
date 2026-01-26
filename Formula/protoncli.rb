class Protoncli < Formula
  desc "Production-ready CLI email client for ProtonMail Bridge"
  homepage "https://github.com/kvnwdev/protoncli"
  url "https://github.com/kvnwdev/protoncli/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "242e27a4f26e0424abce02bac18a019fba98edb5735e30464da3d0d9be7811aa"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "protoncli", shell_output("#{bin}/protoncli --version")
  end
end
