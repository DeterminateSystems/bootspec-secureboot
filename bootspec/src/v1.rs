use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{SpecialisationDescription, SpecialisationName, SystemConfigurationRoot};

/// The V1 bootspec schema version.
pub const SCHEMA_VERSION: u32 = 1;
/// The V1 bootspec schema filename.
pub const JSON_FILENAME: &str = "boot.v1.json";

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// V1 of the bootspec schema.
pub struct BootJsonV1 {
    /// The version of the boot.json schema
    pub schema_version: u32,
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
    /// Mapping of specialisation names to their boot.json (or `None` if it doesn't exist) and their
    /// toplevel
    pub specialisation: HashMap<SpecialisationName, SpecialisationDescription>,
    /// config.system.build.toplevel path
    pub toplevel: SystemConfigurationRoot,
}
