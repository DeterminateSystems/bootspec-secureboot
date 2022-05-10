# bootspec

This repository is a research project that aims to improve the bootloader story in NixOS.

## Crates

### `bootspec`

The `bootspec` crate provides various structures and constants useful for interacting with the NixOS boot specification.

### `synthesize`

The `synthesize` crate provides a CLI that, when provided a path to a NixOS generation and an output directory location, will synthesize a boot specification document from the available information.

The output directory will contain a `boot.v1.json`. This is a stable API. The `boot.v1.json` will point to other files and directories which contain the bootspec for specialisations. These files will also be in the output directory. The names of these other files and directories is NOT considered a stable API, as they should only be accessed via the `boot.v1.json`.

Verify changes to the synthesis tool with `cargo test` and also by running `./cases/verify.sh` to ensure it generates the same results as before.

# License

[MIT](./LICENSE)
