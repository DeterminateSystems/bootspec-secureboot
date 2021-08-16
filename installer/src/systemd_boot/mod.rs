// I'm imagining the installer will create e.g. systemd-boot's loader.conf
// and grub's grub.cfg (amongst other required files) and add it to the
// generated bootloader/ folder, then add it to the store, then update the
// bootloader profile to point to that store path
//
// bootloader profile should only consist of the generated entries?
//
// installer will take generated entries from /nix/var/nix/profiles/bootloader,
// atomically switch all entries (using .tmp and `mv` for systemd), then add
// the bootloader-specific config to the boot partition directly? hardware
// and related config will always be taken from the current system...
//
// a snapshot of the bootloader-specific files that should go into /boot

// collect a list of entries that we generate and remove old ones from ESP/loader/entries:
//     gens = get_generations()
//     for profile in get_profiles():
//         gens += get_generations(profile)
//     remove_old_entries(gens)

/*
1a. only (bare) arg is the path to the default / just-built toplevel
1b. maybe accept flags for stuff like timeout, etc, that goes into the config
2. read machine_id and append it to all entries? (thus removing machine_id handling from the generator... but that's not the best)
3a. NIXOS_INSTALL_GRUB and NIXOS_INSTALL_BOOTLOADER
3b. if N_I_B and loader/loader.conf exists in ESP (destination), remove it
3c. if canTouchEfiVars, bootctl install, else bootctl install --no-variables
4. else, update to latest version of sd-boot (compare systemd and installed sd-boot versions)
5. get a list of entries to generate, also check profiles, and remove old entries
6. write loader conf if one of the generations' store dir (realpath) matches the toplevel we were passed
7. special-case memtest? bleh
8. syncfs to make sure a crash/outage doesn't make the system unbootable
*/

use std::env;
use std::ffi::{CStr, OsString};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use log::{debug, error, info, trace, warn};
use regex::{Regex, RegexBuilder};

use crate::util::{self, Generation};
use crate::{Args, Result};

lazy_static::lazy_static! {
    static ref ENTRY_RE: Regex = Regex::new("nixos-(?:(?P<profile>[^-]+)-)?generation-(?P<generation>\\d+).conf").unwrap();
}

pub(crate) fn install(args: Args) -> Result<()> {
    trace!("beginning systemd_boot install process");

    // systemd_boot requires the path to the ESP be provided, so it's safe to unwrap (until I make
    // this a subcommand and remove the Option wrapper altogether)
    let esp = args.esp.unwrap();
    let bootctl = args.bootctl.unwrap();
    let loader = esp.join("loader/loader.conf");

    // FIXME: support dry run
    // TODO: make a function (macro_rules! macro?) that accepts the potentially-destructive action and a message to log?
    debug!("dry_run? {}", args.dry_run);
    if args.dry_run {
        unimplemented!("dry run still needs to be implemented");
    }

    // purposefully don't support NIXOS_INSTALL_GRUB because it's legacy, and this tool isn't :)
    match env::var("NIXOS_INSTALL_BOOTLOADER") {
        Ok(var) if var == "1" => {
            trace!("installing bootloader");

            if loader.exists() {
                debug!("removing existing loader.conf");
                fs::remove_file(&loader)?;
            }

            debug!("running `bootctl install`");
            Command::new(&bootctl)
                .args(&[
                    "install",
                    &format!("--path={}", &esp.display()),
                    if !args.can_touch_efi_vars {
                        "--no-variables"
                    } else {
                        ""
                    },
                ])
                .status()?;
        }
        _ => {
            trace!("updating bootloader");

            let bootloader_version = {
                trace!("checking bootloader version");

                debug!("running `bootctl status`");
                let output = Command::new(&bootctl)
                    .args(&[&format!("--path={}", &esp.display()), "status"])
                    .output()?
                    .stdout;
                let output = str::from_utf8(&output)?;

                // pat in its own str so that `cargo fmt` doesn't choke...
                let pat = "^\\W+File:.*/EFI/(BOOT|systemd)/.*\\.efi \\(systemd-boot (?P<version>\\d+)\\)$";

                // See enumerate_binaries() in systemd bootctl.c for code which generates this:
                // https://github.com/systemd/systemd/blob/788733428d019791ab9d780b4778a472794b3748/src/boot/bootctl.c#L221-L224
                let re = RegexBuilder::new(pat)
                    .multi_line(true)
                    .case_insensitive(true)
                    .build()?;
                let caps = re.captures(output);

                if let Some(caps) = caps {
                    caps.name("version")
                        .and_then(|cap| cap.as_str().parse::<usize>().ok())
                } else {
                    None
                }
            };

            let systemd_version = {
                trace!("checking systemd version");

                debug!("running `bootctl --version`");
                let output = Command::new(&bootctl).arg("--version").output()?.stdout;
                let output = str::from_utf8(&output)?;

                let re = Regex::new("systemd (?P<version>\\d+) \\(\\d+\\)")?;
                let caps = re.captures(output).expect("");

                caps.name("version")
                    .expect("couldn't find version")
                    .as_str()
                    .parse::<usize>()?
            };

            if let Some(bootloader_version) = bootloader_version {
                if systemd_version > bootloader_version {
                    info!(
                        "updating systemd-boot from {} to {}",
                        bootloader_version, systemd_version
                    );

                    Command::new(&bootctl)
                        .args(&[&format!("--path={}", &esp.display()), "update"])
                        .status()?;
                }
            } else {
                warn!("could not find any previously installed systemd-boot");
            }
        }
    }

    let generations = {
        trace!("getting list of generations");

        let generations = util::all_generations(None)?;
        let generations_len = generations.len();
        debug!("generations_len: {}", generations_len);

        if let Some(limit) = args.configuration_limit {
            debug!("limiting generations to max of {}", limit);

            generations
                .into_iter()
                .skip(generations_len.saturating_sub(limit))
                .collect::<Vec<_>>()
        } else {
            generations
        }
    };

    // Remove old things from both the generated entries and ESP
    // - Generated entries because we don't need to waste space on copying unused kernels / initrds / entries
    // - ESP so that we don't have unbootable entries
    debug!("removing old files from generated_entries");
    self::remove_old_files(&generations, &args.generated_entries)?;
    debug!("removing old files from esp");
    self::remove_old_files(&generations, &esp)?;

    // Reverse the iterator because it's more likely that the generation being switched to is
    // "newer", thus will be at the end of the generated list of generations
    debug!("finding default boot entry by comparing store paths");
    for generation in generations.iter().rev() {
        if fs::canonicalize(&generation.path)? == fs::canonicalize(&args.toplevel)? {
            trace!("writing loader.conf for default boot entry");

            // We don't need to check if loader.conf already exists because we are writing it
            // directly to the `generated_entries` directory (where there cannot be one unless
            // manually placed)
            let gen_loader = args.generated_entries.join("loader/loader.conf");
            let mut f = File::create(&gen_loader)?;

            if let Some(timeout) = args.timeout {
                writeln!(f, "timeout {}", timeout)?;
            }
            // if let Some(profile) = args.profile {
            //     // TODO: support system profiles?
            // } else {
            writeln!(f, "default nixos-generation-{}.conf", generation.idx)?;
            // }
            if !args.editor {
                writeln!(f, "editor 0")?;
            }
            writeln!(f, "console-mode {}", args.console_mode)?;

            break;
        }
    }

    // If there's not enough space for everything, this will error out while copying files, before
    // anything is overwritten via renaming.
    debug!("copying everything to the esp");
    util::atomic_tmp_copy(&args.generated_entries, &esp)?;

    let f = File::open(&esp)?;
    let fd = f.as_raw_fd();

    // TODO
    // SAFETY: idk
    debug!("attempting to syncfs(2) the esp");
    unsafe {
        let ret = libc::syncfs(fd);
        if ret != 0 {
            error!(
                "could not sync '{}': {:?}",
                esp.display(),
                CStr::from_ptr(libc::strerror(ret))
            );
        }
    }

    Ok(())
}

// TODO: split into different binary / subcommand?
fn remove_old_files(generations: &[Generation], esp: &Path) -> Result<()> {
    trace!("removing old files");

    let efi_nixos = esp.join("efi/nixos");
    let loader_entries = esp.join("loader/entries");

    if !esp.exists() || !efi_nixos.exists() || !loader_entries.exists() {
        warn!(
            "'{}', '{}', or '{}' did not exist, not removing anything",
            esp.display(),
            efi_nixos.display(),
            loader_entries.display()
        );

        return Ok(());
    }

    let mut known_paths: Vec<PathBuf> = Vec::new();

    for generation in generations {
        let path = &generation.path;
        known_paths.push(fs::canonicalize(path.join("kernel"))?);
        known_paths.push(fs::canonicalize(path.join("initrd"))?);
    }

    let known_files = known_paths
        .iter()
        .map(|e| {
            let mut s = e
                .to_string_lossy()
                .replace("/nix/store/", "")
                .replace("/", "-");
            s.push_str(".efi");
            s.into()
        })
        .collect::<Vec<OsString>>();

    debug!("removing old entries");
    for entry in fs::read_dir(loader_entries)? {
        let f = entry?.path();
        let name = f
            .file_name()
            .expect("filename terminated in ..")
            .to_string_lossy();

        // Don't want to delete user's custom boot entries
        if !ENTRY_RE.is_match(&name) {
            continue;
        }

        if !generations.iter().any(|e| {
            let caps = ENTRY_RE.captures(&name).unwrap();
            let profile = caps.name("profile").map(|e| e.as_str());
            let idx = caps
                .name("generation")
                .expect("couldn't find generation")
                .as_str()
                .parse::<usize>()
                .expect("couldn't parse generation into number");

            e.idx == idx && e.profile.as_deref() == profile
        }) {
            fs::remove_file(f)?;
        }
    }

    debug!("removing old kernels / initrds");
    for entry in fs::read_dir(efi_nixos)? {
        let f = entry?.path();
        let name = f.file_name().expect("filename terminated in ..");

        if !known_files.iter().any(|e| e == name) {
            fs::remove_file(f)?;
        }
    }

    Ok(())
}
