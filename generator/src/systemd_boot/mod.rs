use std::fs::{self, File};
use std::io::Write;
use std::os::unix;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{BootJson, Result};

mod entry;
pub use entry::entry;

// FIXME: placeholder dir
pub const ROOT: &str = "systemd-boot-entries";

#[derive(Debug)]
pub struct SigningInfo {
    pub signing_key: PathBuf,
    pub signing_cert: PathBuf,
    pub objcopy: PathBuf,
    pub sbsign: PathBuf,
}

pub fn generate(
    json: &BootJson,
    generation: usize,
    profile: &Option<String>,
    generation_path: &Path,
    signing_info: &Option<SigningInfo>,
) -> Result<()> {
    for (path, contents) in entry::entry(json, generation, profile, signing_info.is_some())? {
        let efi_nixos = format!("{}/efi/nixos", self::ROOT);
        let loader_entries = format!("{}/loader/entries", self::ROOT);
        fs::create_dir_all(&efi_nixos)?;
        fs::create_dir_all(&loader_entries)?;

        let mut f = File::create(&path)?;
        write!(f, "{}", contents.conf)?;

        if let Some(signing_info) = signing_info {
            // 1. create unified kernel
            // 2. sign `f`
            // 3. sign unified kernel, move to proper location
            let kernel_param_file = format!("{}/kernel_params-{}", self::ROOT, generation);
            let mut kernel_param = File::create(&kernel_param_file)?;
            write!(
                kernel_param,
                "init={} {}",
                json.init.display(),
                json.kernel_params.join(" ")
            )?;

            self::create_unified_kernel(
                &signing_info.objcopy,
                generation_path,
                &kernel_param_file,
                &contents.unified_dest,
            )?;
            fs::remove_file(&kernel_param_file)?;

            self::sign_file(
                &signing_info.sbsign,
                &signing_info.signing_key,
                &signing_info.signing_cert,
                &contents.unified_dest,
            )?;
        } else {
            if !Path::new(&contents.kernel_dest).exists() {
                unix::fs::symlink(contents.kernel_src, contents.kernel_dest)?;
            }

            if !Path::new(&contents.initrd_dest).exists() {
                unix::fs::symlink(contents.initrd_src, contents.initrd_dest)?;
            }
        }
    }

    Ok(())
}

fn create_unified_kernel(
    objcopy: &Path,
    generation_path: &Path,
    kernel_params: &str,
    unified_kernel_dest: &str,
) -> Result<()> {
    // TODO: use `object` crate instead of calling objcopy?
    // FIXME: (in the future) check that offsets won't overlap; error if they will
    //   - if kernel size is "close" to 16MiB, move initrd offset to 0x4000000

    // Offsets taken from one of systemd's EFI tests:
    // https://github.com/systemd/systemd/blob/01d0123f044d6c090b6ac2f6d304de2bdb19ae3b/test/test-efi-create-disk.sh#L32-L38
    Command::new(objcopy)
        .args(&[
            "--add-section",
            &format!(".osrel={}/etc/os-release", generation_path.display()),
            "--change-section-vma",
            ".osrel=0x20000",
            "--add-section",
            &format!(".cmdline={}", kernel_params),
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
            &format!(
                "{}/sw/lib/systemd/boot/efi/linuxx64.efi.stub",
                generation_path.display()
            ),
            unified_kernel_dest,
        ])
        .status()?;

    Ok(())
}

fn sign_file(sbsign: &Path, key: &Path, cert: &Path, file: &str) -> Result<()> {
    Command::new(sbsign)
        .args(&[
            "--key",
            &key.display().to_string(),
            "--cert",
            &cert.display().to_string(),
            "--output",
            file,
            file,
        ])
        .status()?;

    Ok(())
}
