// this just creates generation boot configs
// accepts a list of system profiles / generations

// TODO: better error handling
//  -> just replace unwraps with expects for now

// to create the bootloader profile:
// (do this in the installer package?)
// 1. cd (mktemp -d)
// 2. run this to get boot/entries/...
// 3. nix-store --add ./somepath
// 4a. make sure bootloader profile doesn't exist (or is ours)?
// 4b. then nix-env -p /nix/var/nix/profiles/bootloader --set ...

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Local, TimeZone};
use regex::Regex;
use serde::{Deserialize, Serialize};

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
    kernel: PathBuf,
    /// Kernel version
    kernel_version: String,
    /// list of kernel parameters
    kernel_params: Vec<String>,
    /// Path to the init script
    init: PathBuf,
    /// Path to initrd -- $toplevel/initrd
    initrd: PathBuf,
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

lazy_static::lazy_static! {
    static ref SYSTEM_RE: Regex = Regex::new("/profiles/system-(?P<generation>\\d+)-link").unwrap();
    static ref PROFILE_RE: Regex = Regex::new("/system-profiles/(?P<profile>[^-]+)-(?P<generation>\\d+)-link").unwrap();
}

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    // TODO: --out-dir?
    // if len(args) < 2, quit
    // this will eventually accept a list of profiles / generations with which to generate bootloader configs
    let generations = env::args().skip(1);
    // basically [/nix/var/nix/profiles/system-69-link, /nix/var/nix/profiles/system-70-link, ...]

    for generation in generations {
        if generation.is_empty() {
            continue;
        }

        let (i, profile) = if PROFILE_RE.is_match(&generation) {
            let caps = PROFILE_RE.captures(&generation).unwrap();
            let i = caps["generation"].parse::<usize>().unwrap();

            (i, Some(caps["profile"].to_string()))
        } else {
            let caps = SYSTEM_RE.captures(&generation).unwrap();
            let i = caps["generation"].parse::<usize>().unwrap();

            (i, None)
        };

        let generation_path = PathBuf::from(&generation);
        let json_path = format!("{}/{}", generation_path.display(), JSON_FILENAME);

        let json: BootJson = if Path::new(&json_path).exists() {
            let contents = std::fs::read_to_string(&json_path).unwrap();
            serde_json::from_str(&contents).unwrap()
        } else {
            synth_data(generation_path).unwrap()
        };

        systemd_entry(&json, i, profile).unwrap();
        // grub_entry(&json, i, profile);
    }
}

// TODO: better name
fn synth_data(generation: PathBuf) -> Result<BootJson> {
    let generation = generation.canonicalize()?;

    let system_version = fs::read_to_string(generation.join("nixos-version"))?;

    let kernel = fs::canonicalize(generation.join("kernel-modules/bzImage"))?;

    let kernel_modules = fs::canonicalize(generation.join("kernel-modules/lib/modules"))?;
    let kernel_glob = fs::read_dir(kernel_modules)?
        .map(|res| res.map(|e| e.path()))
        .next()
        .unwrap()?;
    let kernel_version = kernel_glob.file_name().unwrap().to_str().unwrap();

    let kernel_params: Vec<String> = fs::read_to_string(generation.join("kernel-params"))?
        .split(' ')
        .map(|e| e.to_string())
        .collect();

    let init = generation.join("init");

    let initrd = fs::canonicalize(generation.join("initrd"))?;

    let initrd_secrets = generation.join("append-initrd-secrets");

    let mut specialisation: HashMap<SpecialisationName, BootJsonPath> = HashMap::new();
    for spec in fs::read_dir(generation.join("specialisation"))?.map(|res| res.map(|e| e.path())) {
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

fn systemd_entry(json: &BootJson, generation: usize, profile: Option<String>) -> Result<()> {
    systemd_entry_impl(json, generation, &profile, None)
}

fn systemd_entry_impl(
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

    // FIXME: placeholder dir
    const ROOT: &'static str = "systemd-boot-entries";
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

        systemd_entry_impl(&parsed, generation, &profile, Some(&name.0))?;
    }

    Ok(())
}

fn get_machine_id() -> String {
    let machine_id = if Path::new("/etc/machine-id").exists() {
        fs::read_to_string("/etc/machine-id").expect("error reading machine-id")
    } else {
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

// Generate the entries, but have the installer create the overall grub.cfg
// write to grub.entries file, pass that to the installer?
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
