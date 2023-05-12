{
  description = "Developpment shell for Plume including nightly Rust compiler";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.rust-overlay = {
    url = "github:oxalica/rust-overlay";
    inputs.nixpkgs.follows = "nixpkgs";
  };
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit system overlays; };
      inputs = with pkgs; [
            (rust-bin.nightly.latest.default.override {
              targets = [ "wasm32-unknown-unknown" ];
            })
            wasm-pack
            openssl
            pkg-config
            gettext
            postgresql
            sqlite
          ];
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "plume";
          version = "0.7.3-dev";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "pulldown-cmark-0.8.0" = "sha256-lpfoRDuY3zJ3QmUqJ5k9OL0MEdGDpwmpJ+u5BCj2kIA=";
              "rocket_csrf-0.1.2" = "sha256-WywZfMiwZqTPfSDcAE7ivTSYSaFX+N9fjnRsLSLb9wE=";
            };
          };
          buildNoDefaultFeatures = true;
          buildFeatures = ["postgresql" "s3"];

          nativeBuildInputs = inputs;

          buildPhase = ''
			wasm-pack build --target web --release plume-front
			cargo build --no-default-features --features postgresql,s3 --path .
			cargo build --no-default-features --features postgresql,s3 --path plume-cli
          '';
          installPhase = ''
			cargo install --no-default-features --features postgresql,s3 --path . --target-dir $out
			cargo install --no-default-features --features postgresql,s3 --path plume-cli --target-dir $out
		  '';
        };
        devShells.default = pkgs.mkShell {
          packages = inputs;
        };
      });
}
