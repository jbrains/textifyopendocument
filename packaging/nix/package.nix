{ lib, rustPlatform, fetchFromGitea }:

rustPlatform.buildRustPackage rec {
  pname = "textifyopendocument";
  version = "0.1.0";

  src = fetchFromGitea {
    domain = "codeberg.org";
    owner = "jbrains";
    repo = "textifyopendocument";
    rev = "v${version}";
    # Replace after the tag exists:
    #   nix-prefetch-url --unpack \
    #     https://codeberg.org/jbrains/textifyopendocument/archive/v0.1.0.tar.gz
    hash = lib.fakeHash;
  };

  # Replace with the value `nix build` reports after the first failed build.
  cargoHash = lib.fakeHash;

  meta = with lib; {
    description = "Fast OpenDocument (odt/ods/odp) to plain text converter for previews";
    homepage = "https://codeberg.org/jbrains/textifyopendocument";
    license = licenses.gpl2Only;
    mainProgram = "textifyopendocument";
    maintainers = [ ]; # add yourself after joining the nixpkgs maintainers list
    platforms = platforms.linux;
  };
}
