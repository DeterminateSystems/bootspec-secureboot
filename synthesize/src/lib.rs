use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use bootspec::{
    BootJson, BootSpecPath, SpecialisationDescription, SpecialisationName, SystemConfigurationRoot,
    JSON_FILENAME, SCHEMA_VERSION,
};

#[doc(hidden)]
pub type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

/// Synthesize a [`BootJson`] struct from the path to a generation.
///
/// This is useful when used on generations that do not have a bootspec attached to it.
pub fn synthesize_schema_from_generation(generation: &Path, out_path: &Path) -> Result<()> {
    fs::create_dir(&out_path)?;
    let specialisationdir = out_path.join("specialisation");
    fs::create_dir(&specialisationdir)?;

    let mut toplevelspec = describe_system(&generation)?;

    if let Ok(specialisations) = fs::read_dir(generation.join("specialisation")) {
        for spec in specialisations.map(|res| res.map(|e| e.path())) {
            let spec = spec?;
            let name = spec
                .file_name()
                .ok_or("Could not get name of specialisation dir")?
                .to_str()
                .ok_or("Specialisation dir name was invalid UTF8")?;
            let toplevel = fs::canonicalize(generation.join(format!("specialisation/{}", name)))?;

            let mut boot_json_path = toplevel.join(JSON_FILENAME);
            if !boot_json_path.exists() {
                let specname = specialisationdir.join(format!("{}.json", name));
                let subspec = describe_system(&toplevel)?;
                let pretty = serde_json::to_string_pretty(&subspec)
                    .map_err(|e| format!("Failed to make pretty JSON from bootspec:\n{}", e))?;

                fs::write(&specname, pretty).map_err(|e| {
                    format!("Failed to write JSON to '{}':\n{}", out_path.display(), e)
                })?;
                boot_json_path = specname;
            }
            toplevelspec.specialisation.insert(
                SpecialisationName(name.to_string()),
                SpecialisationDescription {
                    bootspec: BootSpecPath(boot_json_path),
                },
            );
        }
    }

    let pretty = serde_json::to_string_pretty(&toplevelspec)
        .map_err(|e| format!("Failed to make pretty JSON from bootspec:\n{}", e))?;

    fs::write(&out_path.join("boot.v1.json"), pretty)
        .map_err(|e| format!("Failed to write JSON to '{}':\n{}", out_path.display(), e))?;

    Ok(())
}

fn describe_system(generation: &Path) -> Result<BootJson> {
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

    let initrd_secrets = Some(generation.join("append-initrd-secrets"));

    let specialisation: HashMap<SpecialisationName, SpecialisationDescription> = HashMap::new();

    Ok(BootJson {
        schema_version: SCHEMA_VERSION,
        label: format!("NixOS {} (Linux {})", system_version, kernel_version),
        kernel,
        kernel_params,
        init,
        initrd,
        initrd_secrets,
        toplevel: SystemConfigurationRoot(generation),
        specialisation,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::{collections::HashMap, fs};

    use bootspec::{BootJson, SystemConfigurationRoot, JSON_FILENAME, SCHEMA_VERSION};
    use tempfile::TempDir;

    use super::describe_system;

    fn scaffold(
        system_version: &str,
        kernel_version: &str,
        kernel_params: &Vec<String>,
        specialisations: Option<Vec<&str>>,
        specialisations_have_boot_spec: bool,
    ) -> PathBuf {
        let temp_dir = TempDir::new().expect("Failed to create tempdir for test generation");
        let generation = temp_dir.into_path();

        fs::create_dir_all(generation.join("kernel-modules/lib/modules"))
            .expect("Failed to write to test generation");
        fs::create_dir_all(generation.join("specialisation"))
            .expect("Failed to write to test generation");

        fs::write(generation.join("nixos-version"), system_version)
            .expect("Failed to write to test generation");
        fs::write(generation.join("kernel-modules/bzImage"), "")
            .expect("Failed to write to test generation");
        fs::write(
            generation.join(format!("kernel-modules/lib/modules/{}", kernel_version)),
            "",
        )
        .expect("Failed to write to test generation");
        fs::write(generation.join("kernel-params"), kernel_params.join(" "))
            .expect("Failed to write to test generation");
        fs::write(generation.join("init"), "").expect("Failed to write to test generation");
        fs::write(generation.join("initrd"), "").expect("Failed to write to test generation");
        fs::write(generation.join("append-initrd-secrets"), "")
            .expect("Failed to write to test generation");

        if let Some(specialisations) = specialisations {
            for spec_name in specialisations {
                let spec_path = generation.join("specialisation").join(spec_name);
                fs::create_dir_all(&spec_path).expect("Failed to write to test generation");

                if specialisations_have_boot_spec {
                    fs::write(spec_path.join(JSON_FILENAME), "")
                        .expect("Failed to write to test generation");
                }
            }
        }

        generation
    }

    #[test]
    fn no_bootspec_no_specialisation() {
        let system_version = String::from("test-version-1");
        let kernel_version = String::from("1.1.1-test1");
        let kernel_params = [
            "udev.log_priority=3",
            "systemd.unified_cgroup_hierarchy=1",
            "loglevel=4",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

        let generation = scaffold(
            &system_version,
            &kernel_version,
            &kernel_params,
            None,
            false,
        );
        let spec = describe_system(&generation).unwrap();

        assert_eq!(
            spec,
            BootJson {
                schema_version: SCHEMA_VERSION,
                label: "NixOS test-version-1 (Linux 1.1.1-test1)".into(),
                kernel: generation.join("kernel-modules/bzImage"),
                kernel_params,
                init: generation.join("init"),
                initrd: generation.join("initrd"),
                initrd_secrets: Some(generation.join("append-initrd-secrets")),
                specialisation: HashMap::new(),
                toplevel: SystemConfigurationRoot(generation),
            }
        );
    }

    #[test]
    fn no_bootspec_with_specialisation_no_bootspec() {
        let system_version = String::from("test-version-2");
        let kernel_version = String::from("1.1.1-test2");
        let kernel_params = [
            "udev.log_priority=3",
            "systemd.unified_cgroup_hierarchy=1",
            "loglevel=4",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
        let specialisations = vec!["spec1", "spec2"];

        let generation = scaffold(
            &system_version,
            &kernel_version,
            &kernel_params,
            Some(specialisations),
            false,
        );

        describe_system(&generation).unwrap();
    }

    #[test]
    fn with_bootspec_no_specialisation() {
        let system_version = String::from("test-version-3");
        let kernel_version = String::from("1.1.1-test3");
        let kernel_params = [
            "udev.log_priority=3",
            "systemd.unified_cgroup_hierarchy=1",
            "loglevel=4",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

        let generation = scaffold(
            &system_version,
            &kernel_version,
            &kernel_params,
            None,
            false,
        );

        fs::write(generation.join(JSON_FILENAME), "").expect("Failed to write to test generation");

        let spec = describe_system(&generation).unwrap();

        assert_eq!(
            spec,
            BootJson {
                schema_version: SCHEMA_VERSION,
                label: "NixOS test-version-3 (Linux 1.1.1-test3)".into(),
                kernel: generation.join("kernel-modules/bzImage"),
                kernel_params,
                init: generation.join("init"),
                initrd: generation.join("initrd"),
                initrd_secrets: Some(generation.join("append-initrd-secrets")),
                specialisation: HashMap::new(),
                toplevel: SystemConfigurationRoot(generation),
            }
        );
    }

    #[test]
    fn with_bootspec_with_specialisations_with_bootspec() {
        let system_version = String::from("test-version-4");
        let kernel_version = String::from("1.1.1-test4");
        let kernel_params = [
            "udev.log_priority=3",
            "systemd.unified_cgroup_hierarchy=1",
            "loglevel=4",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
        let specialisations = vec!["spec1", "spec2"];

        let generation = scaffold(
            &system_version,
            &kernel_version,
            &kernel_params,
            Some(specialisations),
            true,
        );

        fs::write(generation.join(JSON_FILENAME), "").expect("Failed to write to test generation");

        describe_system(&generation).unwrap();
    }
}
