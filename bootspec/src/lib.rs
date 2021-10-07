use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
/// A wrapper type describing the name of a NixOS specialisation.
pub struct SpecialisationName(pub String);
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// A wrapper type describing the root directory of a NixOS system configuration.
pub struct SystemConfigurationRoot(pub PathBuf);
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// A wrapper type describing the path to the bootspec schema file.
pub struct BootJsonPath(pub PathBuf);

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// V1 of the bootspec schema.
pub struct BootJsonV1 {
    /// The version of the boot.json schema
    pub schema_version: usize,
    /// NixOS version
    pub system_version: String,
    /// Path to kernel (bzImage) -- $toplevel/kernel
    pub kernel: PathBuf,
    /// Kernel version
    pub kernel_version: String,
    /// list of kernel parameters
    pub kernel_params: Vec<String>,
    /// Path to the init script
    pub init: PathBuf,
    /// Path to initrd -- $toplevel/initrd
    pub initrd: PathBuf,
    /// Path to "append-initrd-secrets" script -- $toplevel/append-initrd-secrets
    pub initrd_secrets: PathBuf,
    /// Mapping of specialisation names to their boot.json if it exists, or `None` if it doesn't
    pub specialisation: HashMap<SpecialisationName, Option<BootJsonPath>>,
    /// config.system.build.toplevel path
    pub toplevel: SystemConfigurationRoot,
}

/// The current bootspec schema.
pub type BootJson = BootJsonV1;
/// The current bootspec schema version.
pub const SCHEMA_VERSION: usize = 1;
/// The current bootspec schema filename.
pub const JSON_FILENAME: &str = "boot.v1.json";
