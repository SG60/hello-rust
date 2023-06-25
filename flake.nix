{
  description = "A very basic flake";

  outputs = { self, nixpkgs }:
    let pkgs = nixpkgs.legacyPackages.x86_64-linux;
  in {
    devShells.x86_64-linux = {
      default = pkgs.mkShell { buildInputs = [ ]; };
      k8s = pkgs.mkShell { buildInputs = with pkgs; [ skaffold ]; };
    };
  };
}
