# bootspec

This repository is a research project that aims to improve the bootloader story in NixOS.

## Crates

### `bootspec`

The `bootspec` crate provides various structures and constants useful for interacting with the NixOS boot specification.

### `synthesize`

The `synthesize` crate provides a CLI that, when provided a path to a NixOS generation and an output file location, will synthesize a boot specification document from the available information. 

### `generator`

The `generator` crate provides a CLI that, when provided a list of NixOS profile generations, will generate bootloader configuration for those generations to a bootloader-specific output directory.

At the moment, only `systemd-boot` is supported.

### `installer`

The `installer` crate provides a CLI that will consume the directory created by the `generator` and install the configuration to the boot device.

At the moment, only `systemd-boot` is supported.

## Usage

> **NOTE:** Please note that only `systemd-boot` is supported at this time.

In order to take this repository for a test drive, you must use a Nixpkgs that carries our related patches (please see our [`boot-spec`](https://github.com/DeterminateSystems/nixpkgs/commits/boot-spec) branch on our Nixpkgs fork for a list of these commits). Once you have an appropriately patched Nixpkgs, you may use the following configuration:

```nix
{ pkgs, lib, config, ... }:
{
  boot.loader.manual = {
    enable = true;
    installHook =
      let
        src = pkgs.fetchFromGitHub {
          owner = "DeterminateSystems";
          repo = "bootspec";
          rev = "78ce788c739ff0b6cbab8982d9081c5efe8156f0";
          sha256 = "sHEDerMHX5n5IAwPKyKzjn5KuCSAxR6ynETHcc1XLg8=";
        };

        patched_sbattach = import (src + "/installer/patched-sbattach.nix") { inherit pkgs; };

        experiment = pkgs.rustPlatform.buildRustPackage rec {
          pname = "experiment";
          version = "0";
          inherit src;
          cargoLock.lockFile = src + "/Cargo.lock";
          buildType = "debug";
          dontStrip = true;

          postPatch = ''
            substituteInPlace installer/build.rs \
              --replace "@patched_sbattach@" "${patched_sbattach}"
          '';
        };
      in
      pkgs.writeShellScript "install-bootloader" ''
        set -x
        cd "$(mktemp -d)" || exit 1

        ${experiment}/bin/generator /nix/var/nix/profiles/system-*-link \
          --systemd-machine-id-setup "${pkgs.systemd}/bin/systemd-machine-id-setup"

        ${experiment}/bin/installer \
          --toplevel="$1" \
          --esp="${config.boot.loader.efi.efiSysMountPoint}" \
          ${lib.optionalString config.boot.loader.efi.canTouchEfiVariables "--touch-efi-vars"} \
          --console-mode="${config.boot.loader.systemd-boot.consoleMode}" \
          --timeout="${toString config.boot.loader.timeout}" \
          --bootctl="${pkgs.systemd}/bin/bootctl" \
          ${lib.optionalString (config.boot.loader.systemd-boot.configurationLimit != null) ''--configuration-limit="${toString config.boot.loader.systemd-boot.configurationLimit}"''} \
          --generated-entries="./systemd-boot-entries"
      '';
  };
}
```

# License

[MIT](./LICENSE)
