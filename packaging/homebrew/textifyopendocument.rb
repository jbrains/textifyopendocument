class Textifyopendocument < Formula
  desc "Fast OpenDocument (odt/ods/odp) to plain text converter for previews"
  homepage "https://github.com/jbrains/textifyopendocument"
  url "https://github.com/jbrains/textifyopendocument/archive/v0.1.1.tar.gz"
  sha256 "e326b2e1dbeb4d33b1a0857bf11522d8d5ed78df41d9dd1a520e5541c0a76898"
  license "GPL-2.0-only"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
    man1.install "textifyopendocument.1"
  end

  test do
    assert_match "textifyopendocument 0.1.1", shell_output("#{bin}/textifyopendocument --version")
  end
end
