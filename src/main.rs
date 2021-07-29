// this just creates boot configs for everything
// accepts a list of system profiles / generations

use std::collections::HashMap;
use std::fs::{self};
// use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
// use serde_json::Result;

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
struct SpecialisationName(String);
#[derive(Debug, Default, Deserialize, Serialize)]
struct SystemConfigurationRoot(PathBuf);
#[derive(Debug, Default, Deserialize, Serialize)]
struct BootJsonPath(PathBuf);

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootJsonV1 {
    /// The version of the boot.json schema
    schema_version: usize,
    /// NixOS version
    system_version: String,
    /// Path to kernel (bzImage) -- $toplevel/kernel
    kernel: String,
    /// Kernel version
    kernel_version: String,
    /// list of kernel parameters
    kernel_params: Vec<String>,
    /// Path to the init script
    init: String,
    /// Path to initrd -- $toplevel/initrd
    initrd: String,
    /// Path to "append-initrd-secrets" script -- $toplevel/append-initrd-secrets
    initrd_secrets: String,
    /// Mapping of specialisation names to their configuration's boot.json -- to add all specialisations as a boot entry
    specialisation: HashMap<SpecialisationName, BootJsonPath>,
    /// config.system.build.toplevel path
    toplevel: SystemConfigurationRoot,
}

type BootJson = BootJsonV1;

fn main() {
    // this will eventually accept a list of profiles / generations with which to generate bootloader configs

    // for each entry:
    // let generation = 1;
    let json = std::fs::read_to_string("boot.v1.json").unwrap();
    let parsed: BootJson = serde_json::from_str(&json).unwrap();

    systemd_entry(&parsed);
    // grub_entry(&parsed);
}

fn grub_entry(json: &BootJson) {
    let data = format!(
        r#"menuentry "NixOS - {profile}" {options} {{
{search}
{{extraPerEntryConfig}}
multiboot {{xen}} {{xenparams}} if xen
module {{kernel}} if xen
module {{initrd}} if xen
linux {linux} {params}
initrd {initrd}
}}
"#,
        profile = "Default",
        options = "--unrestricted",
        search = "--set=drive1 --fs-uuid ASJD-NLSA",
        linux = json.kernel,
        params = json.kernel_params.join(" "),
        initrd = json.initrd,
    );

    println!("{}", data);
}

fn systemd_entry(json: &BootJson) {
    let ctime = fs::metadata(&json.toplevel.0).unwrap().ctime();
    let date = Utc.timestamp(ctime, 0).format("%Y-%m-%d");

    let data = format!(
        r#"title NixOS
version Generation {generation} {description}
linux {esp}/efi/nixos/{linux}.efi
initrd {esp}/efi/nixos/{initrd}.efi
options init={init} {params}
machine-id {machine_id}
"#,
        generation = 1,
        description = format!(
            "NixOS {}, Linux Kernel {}, Built on {}",
            json.system_version, json.kernel_version, date
        ),
        linux = json.kernel,
        initrd = json.initrd,
        init = json.init,
        params = json.kernel_params.join(" "),
        machine_id = "asdf", // TODO: get /etc/machine-id or generate with `systemd-machine-id-setup --print`
        esp = "",
    );

    println!("{}", data);
}
