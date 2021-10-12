# bootspec

This crate provides various structures and constants useful for interacting with the NixOS boot specification.

<!-- TODO: link to the RFC once submitted -->

The `BootJson` struct implements the `serde::Deserialize` and `serde::Serialize` traits, making it easy to work with existing bootspec documents as well as creating new ones.
