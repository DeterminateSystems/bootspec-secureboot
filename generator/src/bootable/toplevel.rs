use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use bootspec::{SpecialisationName, SystemConfigurationRoot};
use chrono::{Local, TimeZone};

use crate::Result;

#[derive(Debug, Default)]
pub struct BootableToplevel {
    /// NixOS version
    pub label: String,
    /// Path to kernel (bzImage) -- $toplevel/kernel
    pub kernel: PathBuf,
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
        let date = Local
            .timestamp_opt(ctime, 0)
            .earliest()
            .map(|d| format!("{}", d.format("%Y-%m-%d")))
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::Other,
                "could not convert toplevel ctime to timestamp",
            ))?;
        let description = format!(
            "{label}{specialisation}, Built on {date}",
            specialisation = if let Some(ref specialisation) = self.specialisation_name {
                format!(", Specialisation {}", specialisation.0)
            } else {
                format!("")
            },
            label = self.label,
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
