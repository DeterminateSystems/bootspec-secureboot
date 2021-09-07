use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Local, TimeZone};

use crate::{BootJson, Result, SpecialisationName, SystemConfigurationRoot};

#[derive(Debug, Default)]
pub struct BootableToplevel {
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
    /// config.system.build.toplevel path
    pub toplevel: SystemConfigurationRoot,
    /// Specialisation name (if a specialisation)
    pub specialisation_name: Option<SpecialisationName>,
    /// Generation index
    pub generation: usize,
    /// Generation profile
    pub profile: Option<String>,
}

impl BootableToplevel {
    pub fn title(&self) -> String {
        format!(
            "NixOS{}",
            if let Some(ref specialisation) = self.specialisation_name {
                format!(" ({})", specialisation.0)
            } else {
                String::new()
            }
        )
    }

    pub fn version(&self) -> Result<String> {
        let ctime = fs::metadata(&self.toplevel.0)?.ctime();
        let date = Local.timestamp(ctime, 0).format("%Y-%m-%d");
        let description = format!(
            "NixOS {system_version}{specialisation}, Linux Kernel {kernel_version}, Built on {date}",
            specialisation = if let Some(ref specialisation) = self.specialisation_name {
                format!(", Specialisation {}", specialisation.0)
            } else {
                format!("")
            },
            system_version = self.system_version,
            kernel_version = self.kernel_version,
            date = date,
        );

        let version = format!(
            "Generation {generation} {description}",
            generation = self.generation,
            description = description
        );

        Ok(version)
    }
}

pub struct EfiProgram {
    pub source: BootableToplevel,
}

impl EfiProgram {
    pub fn new(source: BootableToplevel) -> Self {
        Self { source }
    }

    pub fn write_unified_efi(&self, objcopy: &Path, outpath: &Path, stub: &Path) -> Result<()> {
        let generation_path = &self.source.toplevel.0;
        let mut kernel_params_path = PathBuf::from(&outpath);
        kernel_params_path.set_file_name(".kernel_params.tmp");
        let mut kernel_params_file = File::create(&kernel_params_path)?;

        write!(
            kernel_params_file,
            "init={} {}",
            self.source.init.display(),
            self.source.kernel_params.join(" ")
        )?;

        // Offsets taken from one of systemd's EFI tests:
        // https://github.com/systemd/systemd/blob/01d0123f044d6c090b6ac2f6d304de2bdb19ae3b/test/test-efi-create-disk.sh#L32-L38
        Command::new(objcopy)
            .args(&[
                "--add-section",
                &format!(".osrel={}/etc/os-release", generation_path.display()),
                "--change-section-vma",
                ".osrel=0x20000",
                "--add-section",
                &format!(".cmdline={}", kernel_params_path.display()),
                "--change-section-vma",
                ".cmdline=0x30000",
                "--add-section",
                &format!(".linux={}/kernel", generation_path.display()),
                "--change-section-vma",
                ".linux=0x2000000",
                "--add-section",
                &format!(".initrd={}/initrd", generation_path.display()),
                "--change-section-vma",
                ".initrd=0x3000000",
                &stub.display().to_string(),
                &outpath.display().to_string(),
            ])
            .status()?;

        fs::remove_file(kernel_params_path)?;

        Ok(())
    }
}

pub enum Bootable {
    Linux(BootableToplevel),
    Efi(EfiProgram),
}

#[derive(Debug, Default)]
pub struct Generation {
    pub index: usize,
    pub profile: Option<String>,
    pub json: BootJson,
}

pub fn flatten(
    inputs: Vec<Generation>,
    specialisation_name: Option<SpecialisationName>,
) -> Result<Vec<BootableToplevel>> {
    let mut toplevels = Vec::new();

    for input in inputs {
        let specialisation_name = specialisation_name.clone();

        toplevels.push(BootableToplevel {
            system_version: input.json.system_version,
            kernel: input.json.kernel,
            kernel_version: input.json.kernel_version,
            kernel_params: input.json.kernel_params,
            init: input.json.init,
            initrd: input.json.initrd,
            toplevel: input.json.toplevel,
            specialisation_name,
            generation: input.index,
            profile: input.profile.clone(),
        });

        for (name, path) in input.json.specialisation {
            let json = fs::read_to_string(&path.0)?;
            let parsed: BootJson = serde_json::from_str(&json)?;
            let gen = Generation {
                index: input.index,
                profile: input.profile.clone(),
                json: parsed,
            };

            toplevels.extend(self::flatten(vec![gen], Some(name))?);
        }
    }

    Ok(toplevels)
}
