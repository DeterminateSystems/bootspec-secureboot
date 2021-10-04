use std::fs::{self, File};
use std::io::Write;
use std::os::unix;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bootable::{Bootable, BootableToplevel, EfiProgram};
use crate::{Result, SpecialisationName};

// FIXME: placeholder dir
pub const ROOT: &str = "systemd-boot-entries";
const STORE_PATH_PREFIX: &str = "/nix/store/";
const STORE_HASH_LEN: usize = 32;

#[derive(Default, Debug)]
pub struct StorePath(PathBuf);
#[derive(Default, Debug)]
pub struct EspPath(String);

#[derive(Default, Debug)]
pub struct Contents {
    /// The contents of the generation conf file.
    pub conf: String,
    /// The kernel's store path
    pub kernel_src: Option<PathBuf>,
    /// The kernel's destination path (inside the ESP)
    pub kernel_dest: Option<String>,
    /// The initrd's store path
    pub initrd_src: Option<PathBuf>,
    /// The initrd's destination path (inside the ESP)
    pub initrd_dest: Option<String>,
    /// The unified EFI file's destination path (inside the ESP)
    pub unified_dest: Option<String>,
}

pub fn generate(
    bootables: Vec<Bootable>,
    objcopy: Option<PathBuf>,
    systemd_efi_stub: Option<PathBuf>,
    systemd_machine_id_setup: PathBuf,
) -> Result<()> {
    let machine_id = self::get_machine_id(&systemd_machine_id_setup)?;
    let efi_nixos = format!("{}/efi/nixos", self::ROOT);
    let loader_entries = format!("{}/loader/entries", self::ROOT);
    fs::create_dir_all(&efi_nixos)?;
    fs::create_dir_all(&loader_entries)?;

    for bootable in bootables {
        match bootable {
            Bootable::Efi(efi) => {
                let (path, contents) = self::efi_entry_impl(&efi, &machine_id)?;
                let mut f = File::create(path)?;
                write!(f, "{}", contents.conf)?;

                let unified_dest = contents.unified_dest.unwrap();
                let objcopy = objcopy.as_ref().unwrap();
                let systemd_efi_stub = systemd_efi_stub.as_ref().unwrap();

                efi.write_unified_efi(objcopy, Path::new(&unified_dest), systemd_efi_stub)?;
            }
            Bootable::Linux(toplevel) => {
                let (path, contents) = self::linux_entry_impl(&toplevel, &machine_id)?;
                let mut f = File::create(path)?;
                write!(f, "{}", contents.conf)?;

                let kernel_dest = contents.kernel_dest.unwrap();
                let kernel_src = contents.kernel_src.unwrap();
                let initrd_dest = contents.initrd_dest.unwrap();
                let initrd_src = contents.initrd_src.unwrap();

                if !Path::new(&kernel_dest).exists() {
                    unix::fs::symlink(kernel_src, kernel_dest)?;
                }

                if !Path::new(&initrd_dest).exists() {
                    unix::fs::symlink(initrd_src, initrd_dest)?;
                }
            }
        }
    }

    Ok(())
}

fn efi_entry_impl(efi: &EfiProgram, machine_id: &str) -> Result<(String, Contents)> {
    let generation = efi.source.generation_index;
    let profile = &efi.source.profile_name;
    let specialisation = &efi.source.specialisation_name;
    let unified = format!(
        "/efi/nixos/{}.efi",
        &efi.source
            .toplevel
            .0
            .display()
            .to_string()
            .replace(STORE_PATH_PREFIX, "")[..STORE_HASH_LEN]
    );

    let title = efi.source.title();
    let version = efi.source.version()?;
    let data = format!(
        r#"title {title}
version Generation {generation} {version}
efi {efi}
machine-id {machine_id}

"#,
        title = title,
        generation = generation,
        version = version,
        efi = unified,
        machine_id = machine_id,
    );

    let conf_path = self::conf_path(profile, specialisation, generation);
    let unified_dest = format!("{}/{}", self::ROOT, unified);
    let entry = (
        conf_path,
        Contents {
            conf: data,
            unified_dest: Some(unified_dest),
            ..Default::default()
        },
    );

    Ok(entry)
}

fn linux_entry_impl(toplevel: &BootableToplevel, machine_id: &str) -> Result<(String, Contents)> {
    let generation = toplevel.generation_index;
    let profile = &toplevel.profile_name;
    let specialisation = &toplevel.specialisation_name;
    let linux = format!(
        "/efi/nixos/{}.efi",
        toplevel
            .kernel
            .display()
            .to_string()
            .replace(STORE_PATH_PREFIX, "")
            .replace("/", "-")
    );
    let initrd = format!(
        "/efi/nixos/{}.efi",
        toplevel
            .initrd
            .display()
            .to_string()
            .replace(STORE_PATH_PREFIX, "")
            .replace("/", "-")
    );

    let title = toplevel.title();
    let version = toplevel.version()?;
    let data = format!(
        r#"title {title}
version Generation {generation} {version}
linux {linux}
initrd {initrd}
options init={init} {params}
machine-id {machine_id}

"#,
        title = title,
        generation = generation,
        version = version,
        linux = linux,
        initrd = initrd,
        init = toplevel.init.display(),
        params = toplevel.kernel_params.join(" "),
        machine_id = machine_id,
    );

    let conf_path = self::conf_path(profile, specialisation, generation);
    let kernel_dest = format!("{}/{}", ROOT, linux);
    let initrd_dest = format!("{}/{}", ROOT, initrd);
    let entry = (
        conf_path,
        Contents {
            conf: data,
            kernel_src: Some(toplevel.kernel.clone()),
            kernel_dest: Some(kernel_dest),
            initrd_src: Some(toplevel.initrd.clone()),
            initrd_dest: Some(initrd_dest),
            ..Default::default()
        },
    );

    Ok(entry)
}

fn conf_path(
    profile: &Option<String>,
    specialisation: &Option<SpecialisationName>,
    generation: usize,
) -> String {
    let entries_dir = format!("{}/loader/entries", self::ROOT);
    let infix = if let Some(profile) = profile {
        format!("-{}", profile)
    } else {
        String::new()
    };
    let conf_path = if let Some(specialisation) = specialisation {
        // TODO: the specialisation in filename is required (or it conflicts with other entries), does this mess up sorting?
        format!(
            "{}/nixos{}-generation-{}-{}.conf",
            &entries_dir, infix, generation, specialisation.0
        )
    } else {
        format!(
            "{}/nixos{}-generation-{}.conf",
            &entries_dir, infix, generation
        )
    };

    conf_path
}

fn get_machine_id(systemd_machine_id_setup: &Path) -> Result<String> {
    let machine_id = if Path::new("/etc/machine-id").exists() {
        fs::read_to_string("/etc/machine-id")?
    } else {
        let output = Command::new(systemd_machine_id_setup)
            .arg("--print")
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "execution of `{} --print` failed",
                systemd_machine_id_setup.display()
            )
            .into());
        }

        String::from_utf8(output.stdout)?
    };

    Ok(machine_id.trim().to_string())
}
