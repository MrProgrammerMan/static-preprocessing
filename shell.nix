{pkgs}:

pkgs.mkShell {
  nativeBuildInputs = with pkgs.buildPackages; [ rustc cargo ];
}
