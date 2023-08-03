{ pkgs, self }:

let
  inherit pkgs;
in
pkgs.stdenv.mkDerivation {
  buildInputs = with pkgs; [ openssl ];
  nativeBuildInputs = with pkgs; [ buildPackages.pkg-config buildPackages.gcc buildPackages.protobuf ];
  name = "hello rust backend";
  src = ./.;
}
