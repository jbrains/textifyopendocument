{
  description = "Fast OpenDocument (odt/ods/odp) to plain text converter for previews";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      forAllSystems = f:
        nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] f;
    in
    {
      packages = forAllSystems (system: {
        default =
          nixpkgs.legacyPackages.${system}.callPackage ./packaging/nix/package.nix
            { src = self; };
      });
    };
}
