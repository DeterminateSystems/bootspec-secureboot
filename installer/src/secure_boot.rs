use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use log::debug;

use crate::Result;

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct SigningInfo {
    pub signing_key: PathBuf,
    pub signing_cert: PathBuf,
    pub sbsign: PathBuf,
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
