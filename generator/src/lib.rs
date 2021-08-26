use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use regex::Regex;
use serde::{Deserialize, Serialize};

pub mod bootable;
pub mod grub;
pub mod systemd_boot;

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

lazy_static::lazy_static! {
    static ref SYSTEM_RE: Regex = Regex::new("/profiles/system-(?P<generation>\\d+)-link").unwrap();
    static ref PROFILE_RE: Regex = Regex::new("/system-profiles/(?P<profile>[^-]+)-(?P<generation>\\d+)-link").unwrap();
}

pub fn get_json(generation_path: PathBuf) -> BootJson {
    let json_path = generation_path.join(JSON_FILENAME);
    let json: BootJson = if json_path.exists() {
        let contents = fs::read_to_string(&json_path).unwrap();
        serde_json::from_str(&contents).unwrap()
    } else {
        synthesize_schema_from_generation(generation_path).unwrap()
    };

    json
}

pub fn parse_generation(generation: &str) -> Result<(usize, Option<String>)> {
    if PROFILE_RE.is_match(generation) {
        let caps = PROFILE_RE.captures(generation).unwrap();
        let i = caps["generation"].parse::<usize>()?;

        Ok((i, Some(caps["profile"].to_string())))
    } else if SYSTEM_RE.is_match(generation) {
        let caps = SYSTEM_RE.captures(generation).unwrap();
        let i = caps["generation"].parse::<usize>()?;

        Ok((i, None))
    } else {
        Err("generation wasn't a system or profile generation".into())
    }
}

pub fn synthesize_schema_from_generation(generation: PathBuf) -> Result<BootJson> {
    let generation = generation.canonicalize()?;

    let system_version = fs::read_to_string(generation.join("nixos-version"))?;

    let kernel = fs::canonicalize(generation.join("kernel-modules/bzImage"))?;

    let kernel_modules = fs::canonicalize(generation.join("kernel-modules/lib/modules"))?;
    let kernel_glob = fs::read_dir(kernel_modules)?
        .map(|res| res.map(|e| e.path()))
        .next()
        .unwrap()?;
    let kernel_version = kernel_glob.file_name().unwrap().to_str().unwrap();

    let kernel_params: Vec<String> = fs::read_to_string(generation.join("kernel-params"))?
        .split(' ')
        .map(|e| e.to_string())
        .collect();

    let init = generation.join("init");

    let initrd = fs::canonicalize(generation.join("initrd"))?;

    let initrd_secrets = generation.join("append-initrd-secrets");

    let mut specialisation: HashMap<SpecialisationName, BootJsonPath> = HashMap::new();
    for spec in fs::read_dir(generation.join("specialisation"))?.map(|res| res.map(|e| e.path())) {
        let spec = spec?;
        let name = spec.file_name().unwrap().to_str().unwrap();
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
