class Textifyopendocument < Formula
  desc "Fast OpenDocument (odt/ods/odp) to plain text converter for previews"
  homepage "https://codeberg.org/jbrains/textifyopendocument"
  url "https://codeberg.org/jbrains/textifyopendocument/archive/v0.1.0.tar.gz"
  sha256 "REPLACE_AFTER_TAG_PUSH" # brew fetch --build-from-source textifyopendocument, or sha256sum the tarball
  license "GPL-2.0-only"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
    man1.install "textifyopendocument.1"
  end

  test do
    assert_match "textifyopendocument 0.1.0", shell_output("#{bin}/textifyopendocument --version")
  end
end
