use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct SpecialisationName(pub String);
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct SystemConfigurationRoot(pub PathBuf);
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct BootJsonPath(pub PathBuf);

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootJsonV1 {
    /// The version of the boot.json schema
    schema_version: usize,
    /// NixOS version
    system_version: String,
    /// Path to kernel (bzImage) -- $toplevel/kernel
    kernel: PathBuf,
    /// Kernel version
    kernel_version: String,
    /// list of kernel parameters
    kernel_params: Vec<String>,
    /// Path to the init script
    init: PathBuf,
    /// Path to initrd -- $toplevel/initrd
    initrd: PathBuf,
    /// Path to "append-initrd-secrets" script -- $toplevel/append-initrd-secrets
    initrd_secrets: PathBuf,
    /// Mapping of specialisation names to their configuration's boot.json -- to add all specialisations as a boot entry
    specialisation: HashMap<SpecialisationName, BootJsonPath>,
    /// config.system.build.toplevel path
    toplevel: SystemConfigurationRoot,
}

pub type BootJson = BootJsonV1;
pub type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

pub const SCHEMA_VERSION: usize = 1;
pub const JSON_FILENAME: &str = "boot.v1.json";

pub fn synthesize_schema_from_generation(generation: PathBuf) -> Result<BootJson> {
    let generation = generation
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize generation dir:\n{}", e))?;

    let system_version = fs::read_to_string(generation.join("nixos-version"))
        .map_err(|e| format!("Failed to read system version:\n{}", e))?;

    let kernel = fs::canonicalize(generation.join("kernel-modules/bzImage"))
        .map_err(|e| format!("Failed to canonicalize the kernel:\n{}", e))?;

    let kernel_modules = fs::canonicalize(generation.join("kernel-modules/lib/modules"))
        .map_err(|e| format!("Failed to canonicalize the kernel modules dir:\n{}", e))?;
    let kernel_glob = fs::read_dir(kernel_modules)
        .map_err(|e| format!("Failed to read kernel modules dir:\n{}", e))?
        .map(|res| res.map(|e| e.path()))
        .next()
        .ok_or("Could not find kernel version dir")??;
    let kernel_version = kernel_glob
        .file_name()
        .ok_or("Could not get name of kernel version dir")?
        .to_str()
        .ok_or("Kernel version dir name was invalid UTF8")?;

    let kernel_params: Vec<String> = fs::read_to_string(generation.join("kernel-params"))?
        .split(' ')
        .map(|e| e.to_string())
        .collect();

    let init = generation.join("init");

    let initrd = fs::canonicalize(generation.join("initrd"))
        .map_err(|e| format!("Failed to canonicalize the initrd:\n{}", e))?;

    let initrd_secrets = generation.join("append-initrd-secrets");

    let mut specialisation: HashMap<SpecialisationName, BootJsonPath> = HashMap::new();
    for spec in fs::read_dir(generation.join("specialisation"))?.map(|res| res.map(|e| e.path())) {
        let spec = spec?;
        let name = spec
            .file_name()
            .ok_or("Could not get name of specialisation dir")?
            .to_str()
            .ok_or("Specialisation dir name was invalid UTF8")?;
        let boot_json = fs::canonicalize(
            generation.join(format!("specialisation/{}/{}", name, JSON_FILENAME)),
        )?;

        specialisation.insert(
            SpecialisationName(name.to_string()),
            BootJsonPath(boot_json),
        );
    }

    Ok(BootJson {
        schema_version: SCHEMA_VERSION,
        system_version,
        kernel,
        kernel_version: kernel_version.to_string(),
        kernel_params,
        init,
        initrd,
        initrd_secrets,
        toplevel: SystemConfigurationRoot(generation),
        specialisation,
    })
}
