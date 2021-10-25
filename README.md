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

In order to take this repository for a test drive, you must use a Nixpkgs that carries our related patches (please see our [`boot-spec`](https://github.com/DeterminateSystems/nixpkgs/commits/boot-spec) branch on our Nixpkgs fork for a list of these commits).

### Flakes

Use our Nixpkgs branch, add bootspec as an input, and add our module to your configuration:

```nix
# flake.nix
{
  inputs.nixpkgs.url = "github:DeterminateSystems/nixpkgs/boot-spec";
  inputs.bootspec = {
    url = "github:DeterminateSystems/bootspec/main";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, naersk, atuin, promptpass, bash-preexec, nix, bootspec }: {
    nixosConfigurations.nixos = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        bootspec.nixosModules.bootspec
        ./configuration.nix
      ];
    };
  };
}
```

### Without Flakes

Use our fork of Nixpkgs's `boot-spec` branch: https://github.com/DeterminateSystems/nixpkgs/tree/boot-spec.

For example:

```
$ export "NIX_PATH=nixpkgs=https://github.com/DeterminateSystems/nixpkgs/archive/refs/heads/boot-spec.tar.gz:$NIX_PATH"
```

Then create a `bootspec.nix` file which contains:

```nix
let
  bootspecSrc = builtins.fetchGit {
    url = "https://github.com/DeterminateSystems/bootspec.git";
    ref = "main";
  };
in
{
  imports = [ "${bootspecSrc}/nixos-module.nix" ];
  nixpkgs.overlays = [
    (final: prev: {
      bootspec = import bootspecSrc;
    })
  ];
}
```

Then add the `bootspec.nix` to your NixOS system's `configuration.nix`.

Then run `nixos-rebuild switch`.

# License

[MIT](./LICENSE)
