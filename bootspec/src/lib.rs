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
pub struct BootJsonPath(pub PathBuf);

// !!! IMPORTANT: KEEP THESE IN SYNC !!!
/// The current bootspec schema.
pub type BootJson = v1::BootJsonV1;
/// The current bootspec schema version.
pub const SCHEMA_VERSION: u32 = 1;
/// The current bootspec schema filename.
pub const JSON_FILENAME: &str = "boot.v1.json";
