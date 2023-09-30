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

        // Offsets taken from one of systemd's EFI tests:
        // https://github.com/systemd/systemd/blob/01d0123f044d6c090b6ac2f6d304de2bdb19ae3b/test/test-efi-create-disk.sh#L32-L38
        let status = Command::new(ukify)
            .args(&[
                &format!("--linux={}/kernel", generation_path.display()),
                &format!("--initrd={}/initrd", generation_path.display()),
                &format!("--cmdline=@{}", kernel_params.path().display()),
                &format!("--os-release=@{}/etc/os-release", generation_path.display()),
            ])
            .status()?;

        if !status.success() {
            return Err("failed to write unified efi".into());
        }

        Ok(())
    }
}
