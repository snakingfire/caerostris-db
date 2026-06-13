{
  description = "caerostris-db — a graph database engine backed by commodity durable object storage";

  inputs = {
    devenv-root = {
      url = "file+file:///dev/null";
      flake = false;
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";
    nixpkgs-unstable.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    devenv.url = "github:cachix/devenv";

    # Required by devenv's `languages.rust.channel` to provide the toolchain.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = inputs@{ flake-parts, devenv-root, ... }:
    flake-parts.lib.mkFlake { inherit inputs; }
      {
        imports = [
          inputs.devenv.flakeModule
        ];

        systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

        perSystem = { config, self', inputs', pkgs, system, ... }: {
          formatter = pkgs.nixpkgs-fmt;

          devenv.shells.default =
            let
              pkgs-unstable = import inputs.nixpkgs-unstable { system = pkgs.stdenv.system; };
            in
            {
              name = "caerostris-db";

              imports = [ ];

              # Rust toolchain (stable) comes from devenv's `languages.rust`,
              # which pins rustc / cargo / clippy / rustfmt / rust-analyzer
              # together. Non-Nix users get an equivalent via rust-toolchain.toml.
              languages.rust = {
                enable = true;
                channel = "stable";
                components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
              };

              packages = with pkgs; [
                taplo # TOML formatter (Cargo.toml etc.)
                gitleaks # secret scanning (pre-commit)
                pre-commit # git hook runner
                jq
              ] ++ (with pkgs-unstable; [
                cargo-nextest # fast test runner; pulled fresh from unstable
              ]);
            };

        };

        flake = { };
      };
}
