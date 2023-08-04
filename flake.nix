{
  description = "A very basic flake";

  inputs.flake-utils.url = "github:numtide/flake-utils";


  outputs = { self, nixpkgs, flake-utils }:
  # let
  #   pkgs = nixpkgs.legacyPackages.x86_64-linux;
  #   crossPkgs = import nixpkgs { crossSystem = { config = "aarch64-unknown-linux-gnu"; }; system = "x86_64-linux";};
  #
  #   hello-rust = { system, stdenv, pkgs, crossPkgs }:  stdenv.mkDerivation {
  #          inherit system;
  #          buildInputs = with nixpkgs.legacyPackages.${system}; [ openssl ];
  #          nativeBuildInputs = with nixpkgs.legacyPackages.${system}; [ buildPackages.pkg-config buildPackages.gcc ];
  #          name = "hello rust backend";
  #          src = ./.;
  #          builder = "cargo build --target=aarch64-unknown-linux-gnu";
  #   }; 
  # in {
    # packages.aarch64-linux.default = derivation {
    #   name = "simple";
    #   builder = "${nixpkgs.legacyPackages."aarch64-linux".bash}/bin/bash";
    #   args = [ "-c" "cargo build --target=aarch64-unknown-linux-gnu" ];
    #   src = ./.;
    #   system = "aarch64-linux";
    # };
  #   devShells.x86_64-linux = {
  #     default = pkgs.mkShell { buildInputs = with crossPkgs; [ openssl ]; nativeBuildInputs = with crossPkgs; [ buildPackages.pkg-config buildPackages.gcc ]; };
  #     k8s = pkgs.mkShell { buildInputs = with pkgs; [ skaffold ]; };
  #   };
  # }
  # //
  # flake-utils.lib.eachDefaultSystem (system: {
  # packages =
  #     flake-utils.lib.eachSystem (/* nixpkgs.lib.filter (sys: sys != system) */ flake-utils.lib.allSystems) (targetSystem:
  #     {
  #     inherit (pkgs) hello;
  #       cross =
  #         let
  #           pkgs = import nixpkgs { inherit system; };
  #           crossPkgs = import nixpkgs { localSystem = system; crossSystem = targetSystem; };
  #         in
  #         
  #         hello-rust {system= system; stdenv= pkgs.stdenv; pkgs=pkgs; crossPkgs=crossPkgs;};
  #           # stdenv.mkDerivation {
  #           # inherit system
  #           # }
  #         
  #     }
  #     );
  # })
  # //
  flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = import nixpkgs { inherit system; };
    # crossPkgs = import nixpkgs { localSystem = system; crossSystem = targetSystem; };
  in
    {
    packages = {
      default = pkgs.callPackage ./derivation.nix { pkgs=pkgs; self=self; };
    };
    devShells = {
      compile = pkgs.mkShell {
        inputsFrom = [ self.packages.${system}.default ];
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.openssl ];
      };
      default = with pkgs; mkShell {
        buildInputs = [ openssl ];
        nativeBuildInputs = [ protobuf ];
        LD_LIBRARY_PATH = lib.makeLibraryPath [ openssl ];
      };
      k8s = pkgs.mkShell { buildInputs = with pkgs; [ skaffold ]; };
    };

    # packages = {
    #   default = 
    #     let
    #       pkgs = nixpkgs.legacyPackages.${system};
    #       inherit (pkgs) stdenv lib;
    #     in
    #     stdenv.mkDerivation {
    #       inherit system;
    #       buildInputs = with nixpkgs.legacyPackages.${system}; [ openssl ];
    #       nativeBuildInputs = with nixpkgs.legacyPackages.${system}; [ buildPackages.pkg-config buildPackages.gcc ];
    #       name = "hello rust backend";
    #       src = ./.;
    #       builder = "cargo build --target=aarch64-unknown-linux-gnu";
    #     };
    # };

    # overlays.default

    # The legacyPackages imported as overlay allows us to use pkgsCross to
    # cross-compile those packages.
    legacyPackages =
    let
    overlay = final: prev: {
        hello-rust = prev.callPackage ./derivation.nix { };
    };
    in import nixpkgs {
      inherit system;
      overlays = [ overlay ];
      crossOverlays = [ overlay ];
    };
  });
}
