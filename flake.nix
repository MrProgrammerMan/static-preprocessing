{
  description = "A preprocessing library for static files used by web servers.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
  };

  outputs = {self, nixpkgs, ...}: 
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.${system}.default = pkgs.callPackage ./shell.nix {};
  };
}
