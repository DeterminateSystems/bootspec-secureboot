use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use log::debug;

use crate::Result;

#[derive(Debug, PartialEq, PartialOrd, clap::Args)]
pub struct SigningInfo {
    /// Path to signing-key file to use when signing secure boot
    #[clap(long, required = false)]
    pub signing_key: PathBuf,
    /// Path to signing-certificate (x509 certificate) file to use when signing secure boot
    #[clap(long, required = false)]
    pub signing_cert: PathBuf,
    /// Path to secure boot signature file to use when signing secure boot
    #[clap(long, required = false)]
    pub sbsign: PathBuf,
    /// Path to secure boot verification file to use when signing secure boot
    #[clap(long, required = false)]
    pub sbverify: PathBuf,
}

impl SigningInfo {
    pub fn sign_file(&self, file: &Path) -> Result<()> {
        let args = &[
            "--key",
            &self.signing_key.display().to_string(),
            "--cert",
            &self.signing_cert.display().to_string(),
            "--output",
            &file.display().to_string(),
            &file.display().to_string(),
        ];
        debug!("running `{}` with args `{:?}`", self.sbsign.display(), args);
        let status = Command::new(&self.sbsign)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(format!("{} could not be signed", file.display()).into());
        }

        Ok(())
    }

    pub fn verify_file(&self, file: &Path) -> Result<()> {
        let args = &[
            "--cert",
            &self.signing_cert.display().to_string(),
            &file.display().to_string(),
        ];
        debug!(
            "running `{}` with args `{:?}`",
            self.sbverify.display(),
            args
        );
        let status = Command::new(&self.sbverify)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(format!("{} could not be verified", file.display()).into());
        }

        Ok(())
    }
}
