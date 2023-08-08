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
          nativeBuildInputs = [ pkgs.pkgsBuildHost.protobuf ];
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

        cross-targets-to-rust = {
          aarch64-linux = "aarch64-unknown-linux-gnu";
          x86_64-linux = "x86_64-unknown-linux-gnu";
        };
        cross-target-systems = with flake-utils.lib.system; [ aarch64-linux x86_64-linux ];
        # TODO: Can this be merged with the normal compilation (do them all in one set of stuff, to avoid repeating myself?)
        cross-results = map
          (targetSystem:
            let
              rust-target = cross-targets-to-rust.${targetSystem};
              nix-cross-pkgs = import nixpkgs { localSystem = system; crossSystem = targetSystem; };
              toolchain = with fenix.packages.${system}; combine
                [ stable.minimalToolchain targets.${rust-target}.stable.rust-std ];
              craneLib = crane.lib.${system}.overrideToolchain toolchain;
              cross-common-args = {
                inherit src;
                CARGO_BUILD_TARGET = rust-target;
                nativeBuildInputs = [ pkgs.pkgsBuildHost.protobuf ];
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
            { inherit targetSystem dockerImage; bin = hello-rust; }
          ) cross-target-systems;

        # turn the results into a flat format that the nix packages output will accept
        cross-packages = builtins.listToAttrs (lib.lists.concatMap
          (x: [
            { name = "docker/${x.targetSystem}"; value = x.dockerImage; }
            { name = "bin/${x.targetSystem}"; value = x.bin; }
          ]) cross-results);

        # keep proto files
        protoFilter = path: _type: builtins.match ".*proto$" path != null;
        # combine with the default source filter
        combinedCraneSourceFilter = path: type:
          (protoFilter path type) || (craneLib.filterCargoSources path type);
      in
      {
        packages = {
          default = hello-rust;
          inherit dockerImage;
        }
        // cross-packages;
        devShells = {
          default = with pkgs; mkShell {
            nativeBuildInputs = [ pkgsBuildHost.protobuf ];
          };
          k8s = pkgs.mkShell { buildInputs = with pkgs; [ skaffold ]; };
        };

        formatter = nixpkgs.legacyPackages.${system}.nixpkgs-fmt;
      });
}
