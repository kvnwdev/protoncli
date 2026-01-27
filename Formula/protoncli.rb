class Protoncli < Formula
  desc "Production-ready CLI email client for ProtonMail Bridge"
  homepage "https://github.com/kvnwdev/protoncli"
  url "https://github.com/kvnwdev/protoncli/archive/refs/tags/v0.3.0.tar.gz"
  sha256 "11b257146c3b43fd673d21ee9a62cc27dce5929a1fcab333c9b8c7703ef51706"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "protoncli", shell_output("#{bin}/protoncli --version")
  end
end
