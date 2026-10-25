{ lib, rustPlatform, fetchFromGithub, src ? null }:

let
  version = "0.1.2";

  # The flake passes `src = self`, so no hash is needed there. For a
  # future nixpkgs PR (where the source is fetched independently), fill
  # this hash with the value the first `nix build` reports.
  realSrc = if src == null then fetchFromGithub {
    domain = "github.com";
    owner = "jbrains";
    repo = "textifyopendocument";
    rev = "v${version}";
    hash = lib.fakeHash;
  } else src;

in rustPlatform.buildRustPackage {
  pname = "textifyopendocument";
  inherit version;

  src = realSrc;

  # Using the committed Cargo.lock avoids a vendored-dependencies hash
  # entirely.
  cargoLock = {
    lockFile = ./../../Cargo.lock;
  };

  meta = with lib; {
    description = "Fast OpenDocument (odt/ods/odp) to plain text converter for previews";
    homepage = "https://github.com/jbrains/textifyopendocument";
    license = licenses.gpl2Only;
    mainProgram = "textifyopendocument";
    maintainers = [ ]; # add yourself after joining the nixpkgs maintainers list
    platforms = platforms.linux;
  };
}
