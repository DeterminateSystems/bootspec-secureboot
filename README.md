# bootspec

This repository is a research project that aims to improve the bootloader story in NixOS.

## Crates

### `bootspec`

The `bootspec` crate provides various structures and constants useful for interacting with the NixOS boot specification.

### `synthesize`

The `synthesize` crate provides a CLI that, when provided a path to a NixOS generation and an output file location, will synthesize a boot specification document from the available information.

Verify changes to the synthesis tool with `cargo test` and also by running `./cases/verify.sh` to ensure it generates the same results as before.

# License

[MIT](./LICENSE)
