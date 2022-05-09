{
  description = "bootloader-experimentation";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

  outputs =
    { self
    , nixpkgs
    , ...
    } @ inputs:
    let
      nameValuePair = name: value: { inherit name value; };
      genAttrs = names: f: builtins.listToAttrs (map (n: nameValuePair n (f n)) names);
      allSystems = [ "x86_64-linux" "aarch64-linux" "i686-linux" "x86_64-darwin" ];

      forAllSystems = f: genAttrs allSystems (system: f {
        inherit system;
        pkgs = import nixpkgs { inherit system; };
      });
    in
    {
      devShell = forAllSystems ({ system, pkgs, ... }:
        pkgs.mkShell {
          name = "bootspec";

          buildInputs = with pkgs; [
            cargo
            rustc
            clippy
            codespell
            nixpkgs-fmt
            rustfmt
          ];
        });

      packages = forAllSystems
        ({ system, pkgs, ... }:
          {
            package = pkgs.rustPlatform.buildRustPackage rec {
              pname = "bootspec";
              version = "unreleased";

              src = self;

              cargoLock.lockFile = ./Cargo.lock;
            };
          });

      defaultPackage = forAllSystems ({ system, ... }: self.packages.${system}.package);
    };
}
