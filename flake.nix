{
  description = "A very basic flake";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix, crane }:

    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        # crossPkgs = import nixpkgs { localSystem = system; crossSystem = targetSystem; };
        craneLib = crane.lib.${system}.overrideToolchain
          fenix.packages.${system}.stable.minimalToolchain;

        src = lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = combinedCraneSourceFilter;
        };
        inherit (pkgs) lib;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          nativeBuildInputs = [ pkgs.buildPackages.protobuf ];
          buildInputs = [
            # Add additional build inputs here
          ];
          # Additional environment variables can be set directly
          # MY_CUSTOM_VAR = "some value";
        };

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        hello-rust = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          # Don't build any other binary artifacts!
          cargoExtraArgs = "--bin=hello-rust-backend";
        });
        dockerImage = pkgs.dockerTools.streamLayeredImage {
          name = "hello-rust-backend";
          tag = "nix-latest-build-tag";
          contents = [ hello-rust /* pkgs.cacert */ ];
          config = {
            Cmd = [ "${hello-rust}/bin/hello-rust-backend" ];
          };
        };

        cross-targets-map = { "aarch64-multiplatform" = "aarch64-unknown-linux-gnu"; };
        cross-targets = [ "aarch64-multiplatform" ];
        # TODO: Can this be merged with the normal compilation (do them all in one set of stuff, to avoid repeating myself?)
        cross-results = builtins.map
          (name:
            let
              rust-target = cross-targets-map.${name};
              nix-cross-target = name;
              nix-cross-pkgs = pkgs.pkgsCross.${nix-cross-target};
              toolchain = with fenix.packages.${system}; combine
                [ stable.minimalToolchain targets.${rust-target}.stable.rust-std ];
              craneLib = crane.lib.${system}.overrideToolchain toolchain;
              cross-common-args = {
                inherit src;
                CARGO_BUILD_TARGET = rust-target;
                nativeBuildInputs = [ pkgs.buildPackages.protobuf ];
              };
              cargoArtifacts = craneLib.buildDepsOnly cross-common-args;
              hello-rust = craneLib.buildPackage (cross-common-args // {
                inherit cargoArtifacts;
                # Don't build any other binary artifacts!
                cargoExtraArgs = "--bin=hello-rust-backend";
              });
              # TODO: make this output the correct architecture
              dockerImage = pkgs.dockerTools.streamLayeredImage {
                name = "hello-rust-backend";
                tag = "nix-latest-build-tag";
                contents = [ hello-rust /* pkgs.cacert */ ];
                config = {
                  Cmd = [ "${hello-rust}/bin/hello-rust-backend" ];
                };
              };
            in
            { name = name; dockerImage = dockerImage; hello-rust = hello-rust; })
          cross-targets;

        cross-packages = builtins.listToAttrs (builtins.map (value: { name = "bin-${value.name}"; value = value.hello-rust; }) cross-results);
        cross-docker = builtins.listToAttrs (builtins.map (value: { name = "docker-${value.name}"; value = value.dockerImage; }) cross-results);

        # keep proto files
        protoFilter = path: _type: builtins.match ".*proto$" path != null;
        # combine with the default source filter
        combinedCraneSourceFilter = path: type:
          (protoFilter path type) || (craneLib.filterCargoSources path type);
      in
      {
        packages = {
          external-derivation = pkgs.callPackage ./derivation.nix { inherit pkgs self; };
          default = hello-rust;
          inherit dockerImage;
        } // cross-packages // cross-docker;
        devShells = {
          compile = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.openssl ];
          };
          # TODO: provide a version for cross compiling to aarch64-unknown-linux-gnu
          default = with pkgs; mkShell {
            nativeBuildInputs = [ buildPackages.protobuf ];
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
          in
          import nixpkgs {
            inherit system;
            overlays = [ overlay ];
            crossOverlays = [ overlay ];
          };

        formatter = nixpkgs.legacyPackages.${system}.nixpkgs-fmt;
      });
}
