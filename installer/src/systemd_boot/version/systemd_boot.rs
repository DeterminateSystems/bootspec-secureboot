use std::cmp::Ordering;
use std::path::Path;
use std::process::Command;
use std::str;

use log::{debug, trace};
use regex::RegexBuilder;

use super::systemd::SystemdVersion;
use crate::Result;

#[derive(Debug, PartialEq)]
pub struct SystemdBootVersion {
    pub version: usize,
}

impl SystemdBootVersion {
    pub fn new(version: usize) -> Self {
        Self { version }
    }

    fn from_output(output: &[u8]) -> Result<Option<Self>> {
        trace!("parsing `bootctl status` output");

        let output = str::from_utf8(output)?;

        // pat in its own str so that `cargo fmt` doesn't choke...
        let pat = "^\\W+File:.*/EFI/(BOOT|systemd)/.*\\.efi \\(systemd-boot (?P<version>\\d+)\\)$";

        // See enumerate_binaries() in systemd bootctl.c for code which generates this:
        // https://github.com/systemd/systemd/blob/788733428d019791ab9d780b4778a472794b3748/src/boot/bootctl.c#L221-L224
        let re = RegexBuilder::new(pat)
            .multi_line(true)
            .case_insensitive(true)
            .build()?;
        let caps = re.captures(output);

        let version = caps.and_then(|caps| {
            caps.name("version")
                .and_then(|cap| cap.as_str().parse::<usize>().ok())
                .map(Self::new)
        });

        Ok(version)
    }

    pub fn detect_version(bootctl: &Path, esp: &Path) -> Result<Option<Self>> {
        trace!("checking bootloader version");

        let args = &["status", "--path", &esp.display().to_string()];
        debug!("running `{}` with args `{:?}`", &bootctl.display(), &args);
        let output = Command::new(&bootctl).args(args).output()?.stdout;

        let version = Self::from_output(&output)?;

        Ok(version)
    }
}

impl PartialEq<SystemdVersion> for SystemdBootVersion {
    fn eq(&self, other: &SystemdVersion) -> bool {
        self.version == other.version
    }
}

impl PartialOrd<SystemdVersion> for SystemdBootVersion {
    fn partial_cmp(&self, other: &SystemdVersion) -> Option<Ordering> {
        Some(self.version.cmp(&other.version))
    }
}

#[cfg(test)]
mod tests {
    use super::{SystemdBootVersion, SystemdVersion};

    #[test]
    fn test_cmp() {
        assert_eq!(
            SystemdBootVersion::new(246) < SystemdVersion::new(247),
            true
        );
        assert_eq!(
            SystemdBootVersion::new(247) < SystemdVersion::new(246),
            false
        );
        assert_eq!(
            SystemdBootVersion::new(247) < SystemdVersion::new(247),
            false
        );
    }

    #[test]
    fn test_eq() {
        assert_eq!(SystemdBootVersion::new(246), SystemdVersion::new(246));
        assert_ne!(SystemdBootVersion::new(246), SystemdVersion::new(247));
    }

    #[test]
    fn test_from_output() {
        assert_eq!(
            SystemdBootVersion::from_output(
                "         File: └─/EFI/systemd/systemd-bootx64.efi (systemd-boot 247)".as_bytes()
            )
            .unwrap(),
            Some(SystemdBootVersion::new(247))
        );
        assert_eq!(
            SystemdBootVersion::from_output(
                "
System:
     Firmware: UEFI 2.70 (American Megatrends 5.17)
  Secure Boot: disabled
   Setup Mode: setup
 Boot into FW: supported

Current Boot Loader:
      Product: systemd-boot 247
     Features: ✓ Boot counting
               ✓ Menu timeout control
               ✓ One-shot menu timeout control
               ✓ Default entry control
               ✓ One-shot entry control
               ✓ Support for XBOOTLDR partition
               ✓ Support for passing random seed to OS
               ✓ Boot loader sets ESP partition information
          ESP: /dev/disk/by-partuuid/00000000-0000-0000-0000-000000000000
         File: └─/EFI/SYSTEMD/SYSTEMD-BOOTX64.EFI

Random Seed:
 Passed to OS: yes
 System Token: set
       Exists: yes

Available Boot Loaders on ESP:
          ESP: /boot (/dev/disk/by-partuuid/00000000-0000-0000-0000-000000000000)
         File: └─/EFI/systemd/systemd-bootx64.efi (systemd-boot 247)
         File: └─/EFI/BOOT/BOOTX64.EFI (systemd-boot 247)

Boot Loaders Listed in EFI Variables:
        Title: Linux Boot Manager
           ID: 0x0000
       Status: active, boot-order
    Partition: /dev/disk/by-partuuid/00000000-0000-0000-0000-000000000000
         File: └─/EFI/SYSTEMD/SYSTEMD-BOOTX64.EFI

Boot Loader Entries:
        $BOOT: /boot (/dev/disk/by-partuuid/00000000-0000-0000-0000-000000000000)

Default Boot Loader Entry:
        title: NixOS (Generation 1 NixOS 21.11pre-git, Linux Kernel 5.12.19, Built on 1970-01-01)
           id: nixos-generation-1.conf
       source: /boot/loader/entries/nixos-generation-1.conf
      version: Generation 1 NixOS 21.11pre-git, Linux Kernel 5.12.19, Built on 1970-01-01
   machine-id: 00000000000000000000000000000000
        linux: /efi/nixos/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-linux-5.12.19-bzImage.efi
       initrd: /efi/nixos/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-initrd-linux-5.12.19-initrd.efi
      options: init=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-nixos-system-a-21.11-pre/init
"
                .as_bytes()
            )
            .unwrap(),
            Some(SystemdBootVersion::new(247))
        );
        assert_eq!(SystemdBootVersion::from_output(b"").unwrap(), None);
    }
}
