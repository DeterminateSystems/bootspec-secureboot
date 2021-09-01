use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Result;

#[derive(Debug, PartialEq, PartialOrd)]
pub struct SigningInfo {
    pub signing_key: PathBuf,
    pub signing_cert: PathBuf,
    pub sbsign: PathBuf,
    pub sbverify: PathBuf,
}

impl SigningInfo {
    pub fn sign_file(&self, file: &Path) -> Result<()> {
        Command::new(&self.sbsign)
            .args(&[
                "--key",
                &self.signing_key.display().to_string(),
                "--cert",
                &self.signing_cert.display().to_string(),
                "--output",
                &file.display().to_string(),
                &file.display().to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        Ok(())
    }

    pub fn verify_file(&self, file: &Path) -> Result<()> {
        Command::new(&self.sbverify)
            .args(&[
                "--cert",
                &self.signing_cert.display().to_string(),
                &file.display().to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        Ok(())
    }
}
