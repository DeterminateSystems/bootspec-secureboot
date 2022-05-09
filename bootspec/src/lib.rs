use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod v1;

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
/// A wrapper type describing the name of a NixOS specialisation.
pub struct SpecialisationName(pub String);

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// A wrapper type describing the root directory of a NixOS system configuration.
pub struct SystemConfigurationRoot(pub PathBuf);

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// A wrapper type describing the path to the bootspec schema file.
pub struct BootSpecPath(pub PathBuf);

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// Provides the information necessary to consume a specialisation with or without a bootspec.
pub struct SpecialisationDescription {
    /// The optional path to the specialisation's bootspec.
    pub bootspec: BootSpecPath,
}

// !!! IMPORTANT: KEEP `BootJson`, `SCHEMA_VERSION`, and `JSON_FILENAME` IN SYNC !!!
/// The current bootspec schema.
pub type BootJson = v1::BootJsonV1;
/// The current bootspec schema version.
pub const SCHEMA_VERSION: u32 = v1::SCHEMA_VERSION;
/// The current bootspec schema filename.
pub const JSON_FILENAME: &str = v1::JSON_FILENAME;
