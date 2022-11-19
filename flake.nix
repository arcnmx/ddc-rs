{
  description = "DDC/CI monitor control";
  inputs = {
    flakelib.url = "github:flakelib/fl";
    nixpkgs = { };
    rust = {
      url = "github:arcnmx/nixexprs-rust";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    arc = {
      url = "github:arcnmx/nixexprs";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { self, flakelib, nixpkgs, ... }@inputs: let
    nixlib = nixpkgs.lib;
    impure = builtins ? currentSystem;
  in flakelib {
    inherit inputs;
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    devShells = {
      plain = {
        mkShell, hostPlatform
      , enableRust ? true, cargo
      , rustTools ? [ ],
      }: mkShell {
        inherit rustTools;
        nativeBuildInputs = nixlib.optional enableRust cargo;
      };
      stable = { rust'stable, rust'latest, outputs'devShells'plain }: let
        stable = if impure then rust'stable else rust'latest;
      in outputs'devShells'plain.override {
        inherit (stable) mkShell;
        enableRust = false;
      };
      dev = { arc'rustPlatforms, rust'nightly, outputs'devShells'plain }: let
        nightly = arc'rustPlatforms.nightly.hostChannel;
        channel = rust'nightly.override {
          inherit (nightly) date manifestPath;
          rustcDev = true;
        };
      in outputs'devShells'plain.override {
        inherit (channel) mkShell;
        enableRust = false;
        rustTools = [ "rust-analyzer" "rustfmt" ];
      };
      default = { outputs'devShells }: outputs'devShells.plain;
    };
    checks = {
      rustfmt = { rustfmt, cargo, runCommand }: runCommand "cargo-fmt-check" {
        nativeBuildInputs = [ cargo (rustfmt.override { asNightly = true; }) ];
        src = self;
        meta.name = "cargo fmt";
      } ''
        cargo fmt --check \
          --manifest-path $src/Cargo.toml
        touch $out
      '';
      test = { rustPlatform }: rustPlatform.buildRustPackage {
        pname = self.lib.cargoToml.package.name;
        inherit (self.lib.cargoToml.package) version;
        cargoLock.lockFile = ./Cargo.lock;
        src = self;
        buildType = "debug";
        meta.name = "cargo test";
      };
    };
    lib = with nixlib; {
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      inherit (self.lib.cargoToml.package) version;
      releaseTag = "v${self.lib.version}";
    };
    config = rec {
      name = "ddc-rs";
      packages.namespace = [ name ];
      inputs.arc = {
        lib.namespace = [ "arc" ];
        packages.namespace = [ "arc" ];
      };
    };
  };
}
