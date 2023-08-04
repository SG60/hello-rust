{ pkgs, self }:

let
  inherit pkgs;
in
pkgs.stdenv.mkDerivation {
  #   buildInputs = with pkgs; [ openssl ];
  nativeBuildInputs = with pkgs; [ buildPackages.pkg-config buildPackages.gcc buildPackages.protobuf ];
  #   buildPhase = "cargo build";
  #   installPhase = "mkdir -p $out/bin; mv ls-output $out";
  name = "hello rust backend";
  src = ./.;
}
