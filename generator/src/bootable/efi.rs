use std::io::Write;
use std::path::Path;
use std::process::Command;

use tempfile::NamedTempFile;

use super::BootableToplevel;
use crate::Result;

pub struct EfiProgram {
    pub source: BootableToplevel,
}

impl EfiProgram {
    pub fn new(source: BootableToplevel) -> Self {
        Self { source }
    }

    pub fn write_unified_efi(&self, ukify: &Path, outpath: &Path, stub: &Path) -> Result<()> {
        let generation_path = &self.source.toplevel.0;
        let mut kernel_params = NamedTempFile::new()?;

        write!(
            kernel_params,
            "init={} {}",
            self.source.init.display(),
            self.source.kernel_params.join(" ")
        )?;

        let status = Command::new(ukify)
            .args(&[
                "build",
                &format!("--linux={}/kernel", generation_path.display()),
                &format!("--initrd={}/initrd", generation_path.display()),
                &format!("--cmdline=@{}", kernel_params.path().display()),
                &format!("--os-release=@{}/etc/os-release", generation_path.display()),
                &format!("--output={}", outpath.display().to_string()),
            ])
            .status()?;

        if !status.success() {
            return Err("failed to write unified efi".into());
        }

        Ok(())
    }
}
