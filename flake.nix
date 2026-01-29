{
  description = "xchecker Rust workspace (dev shell + build)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";

    crane.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        lib = pkgs.lib;

        rustVersion = "1.89.0";
        rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default;
        rustFmt = pkgs.rust-bin.stable.${rustVersion}.rustfmt;
        rustClippy = pkgs.rust-bin.stable.${rustVersion}.clippy;
        rustSrc = pkgs.rust-bin.stable.${rustVersion}.rust-src;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
        };

        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          cargoExtraArgs = "--all-features";
        });

        xchecker = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          doCheck = false;
        });
      in
      {
        packages.default = xchecker;
        packages.xchecker = xchecker;

        apps.default = flake-utils.lib.mkApp { drv = xchecker; };
        apps.xchecker = flake-utils.lib.mkApp { drv = xchecker; };

        formatter = pkgs.nixpkgs-fmt;

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            rustFmt
            rustClippy
            rustSrc
            pkgs.rust-analyzer
            pkgs.cargo-watch
            pkgs.just
            pkgs.ripgrep
            pkgs.git
            pkgs.pkg-config
          ];
          buildInputs = lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
          RUST_BACKTRACE = "1";
        };
      });
}
