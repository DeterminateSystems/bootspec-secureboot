// this just creates generation boot configs
// accepts a list of system profiles / generations

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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
    init: PathBuf,
    /// Path to initrd -- $toplevel/initrd
    initrd: String,
    /// Path to "append-initrd-secrets" script -- $toplevel/append-initrd-secrets
    initrd_secrets: PathBuf,
    /// Mapping of specialisation names to their configuration's boot.json -- to add all specialisations as a boot entry
    specialisation: HashMap<SpecialisationName, BootJsonPath>,
    /// config.system.build.toplevel path
    toplevel: SystemConfigurationRoot,
}

type BootJson = BootJsonV1;
type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

const SCHEMA_VERSION: usize = 1;
const JSON_FILENAME: &'static str = "boot.v1.json";

fn main() {
    // if len(args) < 2, quit
    // this will eventually accept a list of profiles / generations with which to generate bootloader configs
    let generations = env::args().skip(1);
    // basically [/nix/var/nix/profiles/system-69-link, /nix/var/nix/profiles/system-70-link, ...]

    for generation in generations {
        let generation = generation.strip_suffix('/').unwrap_or(&generation);
        // dbg!(&generation);
        let link = generation
            .strip_prefix("/nix/var/nix/profiles/system-")
            .unwrap_or(generation);
        let i = link
            .strip_suffix("-link")
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap();
        // dbg!(link, i);

        let jsonpath = format!("{}/{}", generation, JSON_FILENAME);
        // let jsonpath = JSON_FILENAME;
        let json: BootJson = if Path::new(&jsonpath).exists() {
            let contents = std::fs::read_to_string(JSON_FILENAME).unwrap();
            serde_json::from_str(&contents).unwrap()
        } else {
            synth_data(PathBuf::from(generation)).unwrap()
        };

        let mut f = File::create(format!("testdir/nixos-generation-{}.conf", i)).unwrap();
        write!(f, "{}", systemd_entry(&json, i, None)).unwrap();

        // generate entries for specialisations
        // TODO: specialisation in filename is required, but will mess up sorting...
        // can we have multiple entries in one file? that would be ideal...
        for (name, path) in &json.specialisation {
            let json = fs::read_to_string(&path.0).unwrap();
            let parsed: BootJson = serde_json::from_str(&json).unwrap();

            let mut f =
                File::create(format!("testdir/nixos-generation-{}-{}.conf", i, name.0)).unwrap();
            write!(f, "{}", systemd_entry(&parsed, i, Some(&name.0))).unwrap();
        }
    }

    // systemd_entry(&json, None);
    // grub_entry(&json);
}

// TODO: better name
fn synth_data(generation: PathBuf) -> Result<BootJson> {
    let generation = generation.canonicalize()?;

    let system_version = fs::read_to_string(generation.join("nixos-version"))?;

    let kernel_path = fs::canonicalize(generation.join("kernel-modules/bzImage"))?;
    let kernel = kernel_path
        .strip_prefix("/nix/store/")?
        .display()
        .to_string()
        .replace("/", "-");

    let kernel_modules = fs::canonicalize(generation.join("kernel-modules/lib/modules"))?;
    let kernel_glob = glob::glob(&format!("{}/*", kernel_modules.display()))?
        .next()
        .unwrap()?;
    let kernel_version = kernel_glob.file_name().unwrap().to_str().unwrap();

    let kernel_params: Vec<String> = fs::read_to_string(generation.join("kernel-params"))?
        .split(' ')
        .map(|e| e.to_string())
        .collect();

    let init = generation.join("init");

    let initrd_path = fs::canonicalize(generation.join("initrd"))?;
    let initrd = initrd_path
        .strip_prefix("/nix/store/")?
        .display()
        .to_string()
        .replace("/", "-");

    let initrd_secrets = generation.join("append-initrd-secrets");

    let mut specialisation: HashMap<SpecialisationName, BootJsonPath> = HashMap::new();
    for spec in glob::glob(&format!(
        "{}/*",
        generation.join("specialisation").display()
    ))? {
        let spec = spec?;
        let name = spec.file_name().unwrap().to_str().unwrap();
        let boot_json = fs::canonicalize(
            generation.join(format!("specialisation/{}/{}", name, JSON_FILENAME)),
        )?;

        specialisation.insert(
            SpecialisationName(name.to_string()),
            BootJsonPath(boot_json),
        );
    }

    Ok(BootJson {
        schema_version: SCHEMA_VERSION,
        system_version: system_version,
        kernel: kernel,
        kernel_version: kernel_version.to_string(),
        kernel_params: kernel_params,
        init: init,
        initrd: initrd,
        initrd_secrets: initrd_secrets,
        toplevel: SystemConfigurationRoot(generation),
        specialisation: specialisation,
    })
}

fn systemd_entry(json: &BootJson, generation: usize, specialisation: Option<&str>) -> String {
    let machine_id = get_machine_id();
    let ctime = fs::metadata(&json.toplevel.0).unwrap().ctime();
    let date = Utc.timestamp(ctime, 0).format("%F");
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

    let data = format!(
        r#"title NixOS
version Generation {generation} {description}
linux /efi/nixos/{linux}.efi
initrd /efi/nixos/{initrd}.efi
options init={init} {params}
machine-id {machine_id}
"#,
        generation = generation,
        description = description,
        linux = json.kernel,
        initrd = json.initrd,
        init = json.init.display(),
        params = json.kernel_params.join(" "),
        machine_id = machine_id,
    );

    let mut out = String::new();
    write!(out, "{}", data).unwrap();

    out
}

// TODO: get /etc/machine-id or generate with `systemd-machine-id-setup --print`
fn get_machine_id() -> String {
    String::from("asdfff")
}

/*
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
*/
