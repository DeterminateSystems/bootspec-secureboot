# bootspec

This repository is a research project that aims to improve the bootloader story in NixOS.

## Crates

### `generator`

The `generator` crate provides a CLI that, when provided a list of NixOS profile generations, will generate bootloader configuration for those generations to a bootloader-specific output directory.

At the moment, only `systemd-boot` is supported.

### `installer`

The `installer` crate provides a CLI that will consume the directory created by the `generator` and install the configuration to the boot device.

At the moment, only `systemd-boot` is supported.

## Usage

> **NOTE:** Please note that only `systemd-boot` is supported at this time.

In order to take this repository for a test drive, you must use a Nixpkgs that carries our related patches (please see our [`boot-spec-unstable`](https://github.com/DeterminateSystems/nixpkgs/commits/boot-spec-unstable) branch on our Nixpkgs fork for a list of these commits).

### Flakes

Use our Nixpkgs branch, add bootspec-secureboot as an input, and add our module to your configuration:

```nix
# flake.nix
{
  inputs.nixpkgs.url = "github:DeterminateSystems/nixpkgs/boot-spec-unstable";
  inputs.bootspec-secureboot = {
    url = "github:DeterminateSystems/bootspec-secureboot/update-to-rfc";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, bootspec-secureboot }: {
    nixosConfigurations.nixos = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        bootspec-secureboot.nixosModules.bootspec-secureboot
        ./configuration.nix
      ];
    };
  };
}
```

### Without Flakes

Use our fork of Nixpkgs's `boot-spec-unstable` branch: https://github.com/DeterminateSystems/nixpkgs/tree/boot-spec-unstable.

For example:

```
$ export "NIX_PATH=nixpkgs=https://github.com/DeterminateSystems/nixpkgs/archive/refs/heads/boot-spec-unstable.tar.gz:$NIX_PATH"
```

Then create a `bootspec-secureboot.nix` file which contains:

```nix
let
  bootspecSecurebootSrc = builtins.fetchGit {
    url = "https://github.com/DeterminateSystems/bootspec-secureboot.git";
    ref = "main";
  };
in
{
  imports = [ "${bootspecSecurebootSrc}/nixos-module.nix" ];
  nixpkgs.overlays = [
    (final: prev: {
      bootspec-secureboot = import bootspecSecurebootSrc;
    })
  ];
}
```

Then add the `bootspec-secureboot.nix` to your NixOS system's `configuration.nix`.

Then run `nixos-rebuild switch`.

### Using Secure Boot

> **NOTE**: Secure Boot functionality is in its early stages, and as such some
> things may or may not work as you might expect.

To use Secure Boot, you will need to import the NixOS module as documented
above, as well as set a few configuration options:

```nix
{
  boot.loader.secureboot = {
    enable = true;
    signingKeyPath = "/path/to/the/signing/key";
    signingCertPath = "/path/to/the/signing/cert";
  };
}
```

The Arch Wiki has a good resource on how to generate these keys yourself, which
can be found at
https://wiki.archlinux.org/title/Unified_Extensible_Firmware_Interface/Secure_Boot#Manual_process.
The signing key and cert configured above are the `db.key` and `db.crt`
mentioned in that resource, but the name doesn't matter. You will need to enroll
the generated `PK.cer`, `KEK.cer`, and `db.cer` as the PK (or Platform Key), KEK
(or Key Exchange Key), and DB (or Signature Database key). At this point, you
should be able to boot using Secure Boot.

# License

[MIT](./LICENSE)
