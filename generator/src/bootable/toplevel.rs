use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use chrono::{Local, TimeZone};

use crate::{Result, SpecialisationName, SystemConfigurationRoot};

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
    pub generation_index: usize,
    /// Generation profile
    pub profile_name: Option<String>,
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
            generation = self.generation_index,
            description = description
        );

        Ok(version)
    }
}
