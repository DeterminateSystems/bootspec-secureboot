use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Local, TimeZone};

use crate::{BootJson, Result};

/// A mapping of file paths to file contents
pub type Entries = HashMap<String, Contents>;

#[derive(Default, Debug)]
pub struct StorePath(PathBuf);
#[derive(Default, Debug)]
pub struct EspPath(String);

#[derive(Default, Debug)]
pub struct Contents {
    /// The contents of the generation conf file.
    pub conf: String,
    /// The kernel's store path
    pub kernel_src: PathBuf,
    /// The kernel's destination path (inside the ESP)
    pub kernel_dest: String,
    /// The initrd's store path
    pub initrd_src: PathBuf,
    /// The initrd's destination path (inside the ESP)
    pub initrd_dest: String,
    /// The unified kernel image's destination path (inside the ESP)
    pub unified_dest: String,
}

pub fn entry(
    json: &BootJson,
    generation: usize,
    profile: &Option<String>,
    signed: bool,
) -> Result<Entries> {
    entry_impl(json, generation, profile, signed, None)
}

fn entry_impl(
    json: &BootJson,
    generation: usize,
    profile: &Option<String>,
    signed: bool,
    specialisation: Option<&str>,
) -> Result<Entries> {
    let machine_id = self::get_machine_id();
    let linux = format!(
        "/efi/nixos/{}.efi",
        json.kernel
            .display()
            .to_string()
            .replace("/nix/store/", "")
            .replace("/", "-")
    );
    let initrd = format!(
        "/efi/nixos/{}.efi",
        json.initrd
            .display()
            .to_string()
            .replace("/nix/store/", "")
            .replace("/", "-")
    );
    let unified = format!(
        "/efi/nixos/{}.efi",
        &json
            .toplevel
            .0
            .display()
            .to_string()
            .replace("/nix/store/", "")[..32]
    );

    let ctime = fs::metadata(&json.toplevel.0)?.ctime();
    let date = Local.timestamp(ctime, 0).format("%Y-%m-%d");
    let description = format!(
        "NixOS {system_version}{specialisation}, Linux Kernel {kernel_version}, Built on {date}{signed}",
        specialisation = if let Some(specialisation) = specialisation {
            format!(", Specialisation {}", specialisation)
        } else {
            format!("")
        },
        signed = if signed { " (Secure Boot)" } else { "" },
        system_version = json.system_version,
        kernel_version = json.kernel_version,
        date = date,
    );

    // The newline at the end of the format string is to ensure that all entries
    // are byte-identical -- before this, running `diff -r /boot/loader/entries
    // [output-dir]/loader/entries` would report missing newlines in the
    // generated entries.
    let data = if signed {
        format!(
            r#"title NixOS
version Generation {generation} {description}
efi {unified}
machine-id {machine_id}
        
"#,
            generation = generation,
            description = description,
            unified = unified,
            machine_id = machine_id,
        )
    } else {
        format!(
            r#"title NixOS
version Generation {generation} {description}
linux {linux}
initrd {initrd}
options init={init} {params}
machine-id {machine_id}

"#,
            generation = generation,
            description = description,
            linux = linux,
            initrd = initrd,
            init = json.init.display(),
            params = json.kernel_params.join(" "),
            machine_id = machine_id,
        )
    };

    let entries_dir = format!("{}/loader/entries", super::ROOT);
    let infix = if let Some(profile) = profile {
        format!("-{}", profile)
    } else {
        String::new()
    };
    let conf_path = if let Some(specialisation) = specialisation {
        // TODO: the specialisation in filename is required (or it conflicts with other entries), does this mess up sorting?
        format!(
            "{}/nixos{}-generation-{}-{}.conf",
            &entries_dir, infix, generation, specialisation
        )
    } else {
        format!(
            "{}/nixos{}-generation-{}.conf",
            &entries_dir, infix, generation
        )
    };

    let kernel_dest = format!("{}/{}", super::ROOT, linux);
    let initrd_dest = format!("{}/{}", super::ROOT, initrd);
    let unified_dest = format!("{}/{}", super::ROOT, unified);

    let mut entries = Entries::new();
    entries.insert(
        conf_path,
        Contents {
            conf: data,
            kernel_src: json.kernel.clone(),
            kernel_dest,
            initrd_src: json.initrd.clone(),
            initrd_dest,
            unified_dest,
        },
    );

    for (name, path) in &json.specialisation {
        let json = fs::read_to_string(&path.0)?;
        let parsed: BootJson = serde_json::from_str(&json)?;

        entries.extend(entry_impl(
            &parsed,
            generation,
            profile,
            signed,
            Some(&name.0),
        )?);
    }

    Ok(entries)
}

fn get_machine_id() -> String {
    let machine_id = if Path::new("/etc/machine-id").exists() {
        fs::read_to_string("/etc/machine-id").expect("error reading machine-id")
    } else {
        // FIXME: systemd-machine-id-setup should be interpolated / substituted
        String::from_utf8(
            Command::new("systemd-machine-id-setup")
                .arg("--print")
                .output()
                .expect("failed to execute systemd-machine-id-setup")
                .stdout,
        )
        .expect("found invalid UTF-8")
    };

    machine_id.trim().to_string()
}
