use std::cmp::Ordering;
use std::path::Path;
use std::process::Command;
use std::str;

use log::{debug, trace};
use regex::Regex;

use super::systemd_boot::SystemdBootVersion;
use crate::Result;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SystemdVersion {
    pub version: usize,
}

impl SystemdVersion {
    pub fn new(version: usize) -> Self {
        Self { version }
    }

    fn from_output(output: &[u8]) -> Result<Self> {
        trace!("parsing `bootctl --version` output");

        let output = str::from_utf8(output)?;

        let re = Regex::new("systemd (?P<version>\\d+) \\(\\d+\\)")?;
        let caps = re.captures(output).ok_or("failed to get capture groups")?;

        let version = caps
            .name("version")
            .ok_or("couldn't find version")?
            .as_str()
            .parse::<usize>()?;

        Ok(Self::new(version))
    }

    pub fn detect_version(bootctl: &Path) -> Result<Self> {
        trace!("checking systemd version");

        let args = &["--version"];
        debug!("running `{}` with args `{:?}`", &bootctl.display(), args);
        let output = Command::new(&bootctl).args(args).output()?.stdout;

        let version = Self::from_output(&output)?;

        Ok(version)
    }
}

impl PartialEq<SystemdBootVersion> for SystemdVersion {
    fn eq(&self, other: &SystemdBootVersion) -> bool {
        self.version == other.version
    }
}

impl PartialOrd<SystemdBootVersion> for SystemdVersion {
    fn partial_cmp(&self, other: &SystemdBootVersion) -> Option<Ordering> {
        Some(self.version.cmp(&other.version))
    }
}

#[cfg(test)]
mod tests {
    use super::{SystemdBootVersion, SystemdVersion};

    #[test]
    fn test_cmp() {
        assert_eq!(
            SystemdVersion::new(246) < SystemdBootVersion::new(247),
            true
        );
        assert_eq!(
            SystemdVersion::new(247) < SystemdBootVersion::new(246),
            false
        );
        assert_eq!(
            SystemdVersion::new(247) < SystemdBootVersion::new(247),
            false
        );
    }

    #[test]
    fn test_eq() {
        assert_eq!(SystemdVersion::new(246), SystemdBootVersion::new(246));
        assert_ne!(SystemdVersion::new(246), SystemdBootVersion::new(247));
    }

    #[test]
    fn test_from_output() {
        assert_eq!(
            SystemdVersion::from_output(b"systemd 247 (247)").unwrap(),
            SystemdVersion::new(247)
        );
        assert_eq!(
            SystemdVersion::from_output("systemd 247 (247)
            +PAM +AUDIT -SELINUX +IMA +APPARMOR +SMACK -SYSVINIT +UTMP +LIBCRYPTSETUP +GCRYPT -GNUTLS +ACL +XZ +LZ4 -ZSTD +SECCOMP +BLKID -ELFUTILS +KMOD +IDN2 -IDN +PCRE2 default-hierarchy=unified
            ".as_bytes()).unwrap(),
            SystemdVersion::new(247)
        );
        assert!(SystemdVersion::from_output(b"systemd (247)").is_err());
        assert!(SystemdVersion::from_output(b"systemc 247 (247)").is_err());
    }
}
