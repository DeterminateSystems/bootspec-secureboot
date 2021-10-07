use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use bootspec::{
    BootJson, BootJsonPath, SpecialisationName, SystemConfigurationRoot, JSON_FILENAME,
    SCHEMA_VERSION,
};

#[doc(hidden)]
pub type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

/// Synthesize a [`BootJson`] struct from the path to a generation.
///
/// This is useful when used on generations that do not have a bootspec attached to it.
pub fn synthesize_schema_from_generation(generation: &Path) -> Result<BootJson> {
    let generation = generation
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize generation dir:\n{}", e))?;

    let system_version = fs::read_to_string(generation.join("nixos-version"))
        .map_err(|e| format!("Failed to read system version:\n{}", e))?;

    let kernel = fs::canonicalize(generation.join("kernel-modules/bzImage"))
        .map_err(|e| format!("Failed to canonicalize the kernel:\n{}", e))?;

    let kernel_modules = fs::canonicalize(generation.join("kernel-modules/lib/modules"))
        .map_err(|e| format!("Failed to canonicalize the kernel modules dir:\n{}", e))?;
    let versioned_kernel_modules = fs::read_dir(kernel_modules)
        .map_err(|e| format!("Failed to read kernel modules dir:\n{}", e))?
        .map(|res| res.map(|e| e.path()))
        .next()
        .ok_or("Could not find kernel version dir")??;
    let kernel_version = versioned_kernel_modules
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

    let mut specialisation: HashMap<SpecialisationName, Option<BootJsonPath>> = HashMap::new();
    for spec in fs::read_dir(generation.join("specialisation"))?.map(|res| res.map(|e| e.path())) {
        let spec = spec?;
        let name = spec
            .file_name()
            .ok_or("Could not get name of specialisation dir")?
            .to_str()
            .ok_or("Specialisation dir name was invalid UTF8")?;
        let boot_json_path = generation.join(format!("specialisation/{}/{}", name, JSON_FILENAME));

        let boot_path = if boot_json_path.exists() {
            Some(fs::canonicalize(&boot_json_path)?)
        } else {
            None
        };

        specialisation.insert(
            SpecialisationName(name.to_string()),
            boot_path.map(BootJsonPath),
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
