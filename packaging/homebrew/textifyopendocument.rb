class Textifyopendocument < Formula
  desc "Fast OpenDocument (odt/ods/odp) to plain text converter for previews"
  homepage "https://github.com/jbrains/textifyopendocument"
  url "https://github.com/jbrains/textifyopendocument/archive/v0.1.2.tar.gz"
  sha256 "ea4a6070f47ce155330534de04155cb22fb6a7f1c9c2fc3de20e957e21ed1ca1"
  license "GPL-2.0-only"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
    man1.install "textifyopendocument.1"
  end

  test do
    assert_match "textifyopendocument 0.1.2", shell_output("#{bin}/textifyopendocument --version")
  end
end
