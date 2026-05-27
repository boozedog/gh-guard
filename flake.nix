{
  description = "gh-guard — a security wrapper around the GitHub CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        gh-guard = pkgs.rustPlatform.buildRustPackage {
          pname = "gh-guard";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = with pkgs; [
            openssl
          ] ++ lib.optionals stdenv.isDarwin [
            apple-sdk
          ];
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };
      in
      {
        packages.default = gh-guard;
        devShells.default = pkgs.mkShell {
          inputsFrom = [ gh-guard ];
          packages = with pkgs; [
            cargo
            clippy
            rustfmt
            rust-analyzer
          ];
        };
      });
}
