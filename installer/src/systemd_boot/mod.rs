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
use std::ffi::CStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::Command;

use regex::{Regex, RegexBuilder};

use crate::{util, Args, Result};

pub(crate) fn install(args: Args) -> Result<()> {
    // systemd_boot requires the path to the ESP be provided, so it's safe to unwrap (until I make
    // this a subcommand and remove the Option wrapper altogether)
    let esp = args.esp.unwrap();
    let bootctl = args.bootctl.unwrap();
    let loader = format!("{}/loader/loader.conf", esp.display());

    // FIXME: support dry run
    if args.dry_run {
        unimplemented!("dry run still needs to be implemented");
    }

    // purposefully don't support NIXOS_INSTALL_GRUB because it's legacy, and this tool isn't :)
    match env::var("NIXOS_INSTALL_BOOTLOADER") {
        Ok(var) if var == "1" => {
            if Path::new(&loader).exists() {
                fs::remove_file(&loader)?;
            }

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
            let bootloader_version = {
                let output = Command::new(&bootctl)
                    .args(&[&format!("--path={}", &esp.display()), "status"])
                    .output()?
                    .stdout;
                let output = std::str::from_utf8(&output)?;

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
                let output = Command::new(&bootctl).arg("--version").output()?.stdout;
                let output = std::str::from_utf8(&output)?;

                let re = Regex::new("systemd (?P<version>\\d+) \\(\\d+\\)")?;
                let caps = re.captures(output).expect("");

                caps.name("version").unwrap().as_str().parse::<usize>()?
            };

            if let Some(bootloader_version) = bootloader_version {
                if systemd_version > bootloader_version {
                    writeln!(
                        io::stdout(),
                        "updating systemd-boot from {} to {}",
                        bootloader_version,
                        systemd_version
                    )?;

                    Command::new(&bootctl)
                        .args(&[&format!("--path={}", &esp.display()), "update"])
                        .status()?;
                }
            } else {
                writeln!(
                    io::stdout(),
                    "could not find any previously installed systemd-boot"
                )?;
            }
        }
    }

    // TODO: remove old entries?
    // TODO: verify there's enough space on the device
    util::atomic_recursive_copy(&args.generated_entries, &esp)?;

    for (idx, generation) in all_generations(None)? {
        if fs::canonicalize(&generation)? == fs::canonicalize(&args.toplevel)? {
            let tmp_loader = format!("{}/loader/loader.conf.tmp", esp.display());
            let mut f = File::create(&tmp_loader)?;

            if let Some(timeout) = args.timeout {
                writeln!(f, "timeout {}", timeout)?;
            }
            // if let Some(profile) = args.profile {
            //     // TODO: support system profiles?
            // } else {
            writeln!(f, "default nixos-generation-{}.conf", idx)?;
            // }
            // if let Some(editor) = args.editor {
            //     // TODO
            // }
            writeln!(f, "console-mode {}", args.console_mode)?;

            fs::rename(tmp_loader, &loader)?;

            break;
        }
    }

    // TODO
    // SAFETY: idk
    unsafe {
        let f = File::open(&esp)?;
        let fd = f.as_raw_fd();
        let ret = libc::syncfs(fd);

        if ret != 0 {
            writeln!(
                io::stderr(),
                "could not sync {}: {:?}",
                esp.display(),
                CStr::from_ptr(libc::strerror(ret))
            )?;
        }
    }

    Ok(())
}

fn all_generations(profile: Option<String>) -> Result<Vec<(usize, String)>> {
    let profile_path = if let Some(profile) = profile {
        format!("/nix/var/nix/profiles/system-profiles/{}", profile)
    } else {
        String::from("/nix/var/nix/profiles/system")
    };

    let output = String::from_utf8(
        Command::new("nix-env")
            .args(&["-p", &profile_path, "--list-generations"])
            .output()?
            .stdout,
    )?;

    let mut generations = Vec::new();
    for line in output.lines() {
        let generation = line
            .trim()
            .split(' ')
            .next()
            .expect("couldn't find generation number");

        generations.push((
            generation.parse()?,
            format!("{}-{}-link", profile_path, generation),
        ));
    }

    Ok(generations)
}
