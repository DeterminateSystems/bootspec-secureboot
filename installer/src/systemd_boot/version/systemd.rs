use std::path::Path;
use std::process::Command;
use std::str;

use log::{debug, trace};
use regex::Regex;

use crate::Result;

#[derive(Debug, PartialEq, Clone)]
pub struct SystemdVersion {
    pub version: String,
}

impl SystemdVersion {
    pub fn new(version: impl ToString) -> Self {
        Self {
            version: version.to_string(),
        }
    }

    fn from_output(output: &[u8]) -> Result<Self> {
        trace!("parsing `bootctl --version` output");

        let output = str::from_utf8(output)?;

        let re = Regex::new("systemd [^\\s]+ \\((?P<version>[^\\)]+)\\)")?;
        let caps = re.captures(output).ok_or("failed to get capture groups")?;

        let version = caps
            .name("version")
            .ok_or("couldn't find version")?
            .as_str()
            .parse::<String>()?;

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

#[cfg(test)]
mod tests {
    use super::SystemdVersion;

    #[test]
    fn test_from_output() {
        assert_eq!(
            SystemdVersion::from_output(b"systemd 247 (247)").unwrap(),
            SystemdVersion::new("247")
        );
        assert_eq!(
            SystemdVersion::from_output("systemd 247 (247)
            +PAM +AUDIT -SELINUX +IMA +APPARMOR +SMACK -SYSVINIT +UTMP +LIBCRYPTSETUP +GCRYPT -GNUTLS +ACL +XZ +LZ4 -ZSTD +SECCOMP +BLKID -ELFUTILS +KMOD +IDN2 -IDN +PCRE2 default-hierarchy=unified
            ".as_bytes()).unwrap(),
            SystemdVersion::new("247")
        );
        assert_eq!(
            SystemdVersion::from_output(b"systemd 249 (249.4)").unwrap(),
            SystemdVersion::new("249.4")
        );
        assert_eq!(
            SystemdVersion::from_output(b"systemd 249 (247.4-2-arch)").unwrap(),
            SystemdVersion::new("247.4-2-arch")
        );

        assert!(SystemdVersion::from_output(b"systemd (247)").is_err());
        assert!(SystemdVersion::from_output(b"systemc 247 (247)").is_err());
    }
}
