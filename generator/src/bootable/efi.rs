use std::io::Seek;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::NamedTempFile;

use super::{BootableToplevel, PcrPhase};
use crate::Result;

pub struct EfiProgram {
    pub source: BootableToplevel,
}

impl EfiProgram {
    pub fn new(source: BootableToplevel) -> Self {
        Self { source }
    }

    pub fn write_unified_efi(
        &self,
        objcopy: &Path,
        systemd_measure: &Option<PathBuf>,
        pcr_phases: &Option<Vec<PcrPhase>>,
        outpath: &Path,
        stub: &Path,
    ) -> Result<()> {
        let generation_path = &self.source.toplevel.0;
        let mut kernel_params = NamedTempFile::new()?;
        let mut pcr_sig = NamedTempFile::new()?;

        write!(
            kernel_params,
            "init={} {}",
            self.source.init.display(),
            self.source.kernel_params.join(" ")
        )?;

        write!(pcr_sig, "{{}}")?;

        if let Some(pcr_phases) = pcr_phases {
            for phase in pcr_phases {
                let mut cmd = Command::new(systemd_measure.as_ref().unwrap());
                for bank in &phase.banks {
                    cmd.args(["--bank", bank]);
                }
                cmd.args([
                    "--osrel",
                    &format!("{}/etc/os-release", generation_path.display()),
                    "--cmdline",
                    &format!("{}", kernel_params.path().display()),
                    "--linux",
                    &format!("{}/kernel", generation_path.display()),
                    "--initrd",
                    &format!("{}/initrd", generation_path.display()),
                    "--phase",
                    &phase.phase_path.to_string(),
                    "--private-key",
                    &format!("{}", phase.private_key_file.display()),
                    "--public-key",
                    &format!("{}", phase.public_key_file.display()),
                    "--append",
                    &format!("{}", pcr_sig.path().display()),
                    "sign",
                ]);
                let output = cmd.stderr(Stdio::inherit()).output()?;

                if !output.status.success() {
                    return Err("failed to sign measurement".into());
                }
                pcr_sig.rewind()?;
                pcr_sig.as_file().set_len(0)?;
                pcr_sig.write_all(&output.stdout)?;
            }
        }

        // Offsets taken from one of systemd's EFI tests:
        // https://github.com/systemd/systemd/blob/01d0123f044d6c090b6ac2f6d304de2bdb19ae3b/test/test-efi-create-disk.sh#L32-L38
        let mut cmd = Command::new(objcopy);
        cmd.args([
            "--add-section",
            &format!(".osrel={}/etc/os-release", generation_path.display()),
            "--change-section-vma",
            ".osrel=0x20000",
            "--add-section",
            &format!(".cmdline={}", kernel_params.path().display()),
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
        ]);
        if pcr_phases.is_some() {
            cmd.args([
                "--add-section",
                &format!(".pcrsig={}", pcr_sig.path().display()),
                "--change-section-vma",
                ".pcrsig=0x40000",
            ]);
        }
        cmd.args([&stub.display().to_string(), &outpath.display().to_string()]);
        let status = cmd.status()?;

        if !status.success() {
            return Err("failed to write unified efi".into());
        }

        Ok(())
    }
}
