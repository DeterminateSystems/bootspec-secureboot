use std::fs::{self, File};
use std::io::Write;
use std::os::unix::{self, fs::MetadataExt};
use std::path::Path;
use std::process::Command;

use chrono::{Local, TimeZone};

use crate::{BootJson, Result};

// FIXME: placeholder dir
const ROOT: &str = "systemd-boot-entries";

pub(crate) fn entry(json: &BootJson, generation: usize, profile: &Option<String>) -> Result<()> {
    entry_impl(json, generation, profile, None)
}

fn entry_impl(
    json: &BootJson,
    generation: usize,
    profile: &Option<String>,
    specialisation: Option<&str>,
) -> Result<()> {
    let machine_id = get_machine_id();
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

    let ctime = fs::metadata(&json.toplevel.0)?.ctime();
    let date = Local.timestamp(ctime, 0).format("%Y-%m-%d");
    let description = format!(
        "NixOS {system_version}{specialisation}, Linux Kernel {kernel_version}, Built on {date}",
        specialisation = if let Some(specialisation) = specialisation {
            format!(", Specialisation {}", specialisation)
        } else {
            format!("")
        },
        system_version = json.system_version,
        kernel_version = json.kernel_version,
        date = date,
    );

    // The newline at the end of the format string is to ensure that all entries
    // are byte-identical -- before this, running `diff -r /boot/loader/entries
    // [output-dir]/loader/entries` would report missing newlines in the
    // generated entries.
    let data = format!(
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
    );

    let entries_dir = format!("{}/loader/entries", ROOT);
    let nixos_dir = format!("{}/efi/nixos", ROOT);
    fs::create_dir_all(&entries_dir)?;
    fs::create_dir_all(&nixos_dir)?;

    let infix = if let Some(profile) = profile {
        format!("-{}", profile)
    } else {
        String::new()
    };

    let mut f = if let Some(specialisation) = specialisation {
        // TODO: the specialisation in filename is required (or it conflicts with other entries), does this mess up sorting?
        File::create(format!(
            "{}/nixos{}-generation-{}-{}.conf",
            &entries_dir, infix, generation, specialisation
        ))?
    } else {
        File::create(format!(
            "{}/nixos{}-generation-{}.conf",
            &entries_dir, infix, generation
        ))?
    };

    write!(f, "{}", data)?;

    let kernel_dest = format!("{}/{}", ROOT, linux);
    if !Path::new(&kernel_dest).exists() {
        unix::fs::symlink(&json.kernel, kernel_dest)?;
    }

    let initrd_dest = format!("{}/{}", ROOT, initrd);
    if !Path::new(&initrd_dest).exists() {
        unix::fs::symlink(&json.initrd, initrd_dest)?;
    }

    for (name, path) in &json.specialisation {
        let json = fs::read_to_string(&path.0)?;
        let parsed: BootJson = serde_json::from_str(&json)?;

        entry_impl(&parsed, generation, profile, Some(&name.0))?;
    }

    Ok(())
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
