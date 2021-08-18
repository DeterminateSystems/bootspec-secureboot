use std::ffi::{CStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::Command;

use log::{debug, error, info, trace, warn};
use regex::Regex;

use crate::util::{self, Generation};
use crate::{Args, Result};
use version::systemd::SystemdVersion;
use version::systemd_boot::SystemdBootVersion;

mod version;

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

    if args.install {
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
    } else {
        trace!("updating bootloader");

        let bootloader_version = SystemdBootVersion::detect_version(&bootctl, &esp)?;
        let systemd_version = SystemdVersion::detect_version(&bootctl)?;

        if let Some(bootloader_version) = bootloader_version {
            if bootloader_version < systemd_version {
                info!(
                    "updating systemd-boot from {} to {}",
                    bootloader_version.version, systemd_version.version
                );

                Command::new(&bootctl)
                    .args(&["update", "--path", &esp.display().to_string()])
                    .status()?;
            }
        } else {
            warn!("could not find any previously installed systemd-boot");
        }
    }

    let system_generations = util::all_generations(None)?;
    let wanted_generations =
        self::wanted_generations(system_generations, args.configuration_limit)?;

    // Remove old things from both the generated entries and ESP
    // - Generated entries because we don't need to waste space on copying unused kernels / initrds / entries
    // - ESP so that we don't have unbootable entries
    debug!("removing old files from generated_entries");
    self::remove_old_files(&wanted_generations, &args.generated_entries)?;
    debug!("removing old files from esp");
    self::remove_old_files(&wanted_generations, &esp)?;

    // Reverse the iterator because it's more likely that the generation being switched to is
    // "newer", thus will be at the end of the generated list of generations
    debug!("finding default boot entry by comparing store paths");
    for generation in wanted_generations.iter().rev() {
        if fs::canonicalize(&generation.path)? == fs::canonicalize(&args.toplevel)? {
            trace!("writing loader.conf for default boot entry");

            // We don't need to check if loader.conf already exists because we are writing it
            // directly to the `generated_entries` directory (where there cannot be one unless
            // manually placed)
            let gen_loader = args.generated_entries.join("loader/loader.conf");
            let mut f = File::create(&gen_loader)?;
            let contents = self::create_loader_conf(
                args.timeout,
                generation.idx,
                args.editor,
                args.console_mode,
            )?;

            f.write_all(contents.as_bytes())?;

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

pub(crate) fn create_loader_conf(
    timeout: Option<usize>,
    idx: usize,
    editor: bool,
    console_mode: String,
) -> Result<String> {
    let mut s = String::new();

    if let Some(timeout) = timeout {
        writeln!(s, "timeout {}", timeout)?;
    }
    // if let Some(profile) = profile {
    //     // TODO: support system profiles?
    // } else {
    writeln!(s, "default nixos-generation-{}.conf", idx)?;
    // }
    if !editor {
        writeln!(s, "editor 0")?;
    }
    writeln!(s, "console-mode {}", console_mode)?;

    Ok(s)
}

pub(crate) fn wanted_generations(
    generations: Vec<Generation>,
    configuration_limit: Option<usize>,
) -> Result<Vec<Generation>> {
    trace!("getting list of generations");

    let generations_len = generations.len();
    debug!("generations_len: {}", generations_len);

    let generations = if let Some(limit) = configuration_limit {
        debug!("limiting generations to max of {}", limit);

        generations
            .into_iter()
            .skip(generations_len.saturating_sub(limit))
            .collect::<Vec<_>>()
    } else {
        generations
    };

    Ok(generations)
}

pub(crate) fn get_required_file_paths(generations: Vec<Generation>) -> Result<Vec<OsString>> {
    let mut known_paths = Vec::new();

    for generation in generations {
        known_paths.push(generation.conf_filename);
        known_paths.push(generation.kernel_filename);
        known_paths.push(generation.initrd_filename);
    }

    Ok(known_paths)
}

// TODO: split into different binary / subcommand?
fn remove_old_files(generations: &[Generation], path: &Path) -> Result<()> {
    trace!("removing old files");

    let efi_nixos = path.join("efi/nixos");
    let loader_entries = path.join("loader/entries");

    if !path.exists() || !efi_nixos.exists() || !loader_entries.exists() {
        warn!(
            "'{}', '{}', or '{}' did not exist, not removing anything",
            path.display(),
            efi_nixos.display(),
            loader_entries.display()
        );

        return Ok(());
    }

    debug!("calculating required file paths");
    let required_file_paths = self::get_required_file_paths(generations.to_vec())?;

    debug!("removing old entries");
    for entry in fs::read_dir(loader_entries)? {
        let f = entry?.path();
        let name = f.file_name().ok_or("filename terminated in ..")?;

        // Don't want to delete user's custom boot entries
        if !ENTRY_RE.is_match(&name.to_string_lossy()) {
            continue;
        }

        if !required_file_paths.iter().any(|e| e == name) {
            fs::remove_file(f)?;
        }
    }

    debug!("removing old kernels / initrds");
    for entry in fs::read_dir(efi_nixos)? {
        let f = entry?.path();
        let name = f.file_name().ok_or("filename terminated in ..")?;

        if !required_file_paths.iter().any(|e| e == name) {
            fs::remove_file(f)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::util::Generation;
    use std::ffi::OsString;

    #[test]
    fn test_create_bootloader_config() {
        assert_eq!(
            super::create_loader_conf(Some(1), 125, true, String::from("max")).unwrap(),
            r#"timeout 1
default nixos-generation-125.conf
console-mode max
"#
        );
        assert_eq!(
            super::create_loader_conf(Some(2), 126, false, String::from("max")).unwrap(),
            r#"timeout 2
default nixos-generation-126.conf
editor 0
console-mode max
"#
        );
    }

    #[test]
    fn test_wanted_generations() {
        let gens = [
            vec![
                Generation {
                    idx: 1,
                    profile: None,
                    conf_filename: OsString::from("nixos-generation-1.conf"),
                    ..Default::default()
                },
                Generation {
                    idx: 2,
                    profile: None,
                    conf_filename: OsString::from("nixos-generation-2.conf"),
                    ..Default::default()
                },
            ],
            vec![
                Generation {
                    idx: 1,
                    profile: Some(String::from("test")),
                    conf_filename: OsString::from("nixos-test-generation-1.conf"),
                    ..Default::default()
                },
                Generation {
                    idx: 2,
                    profile: Some(String::from("test")),
                    conf_filename: OsString::from("nixos-test-generation-2.conf"),
                    ..Default::default()
                },
            ],
        ];

        for generations in gens {
            let ret_generations = super::wanted_generations(generations.clone(), None).unwrap();
            assert_eq!(ret_generations.len(), 2);
            assert_eq!(ret_generations[0], generations[0]);
            assert_eq!(ret_generations[1], generations[1]);

            let ret_generations = super::wanted_generations(generations.clone(), Some(1)).unwrap();
            assert_eq!(ret_generations.len(), 1);
            assert_eq!(ret_generations[0], generations[1]);
            assert_eq!(ret_generations.get(1), None);
        }
    }

    #[test]
    fn test_get_known_filenames() {
        let generations = vec![
            Generation {
                idx: 1,
                profile: None,
                conf_filename: OsString::from("nixos-generation-1.conf"),
                kernel_filename: OsString::from("kernel-1"),
                initrd_filename: OsString::from("initrd-1"),
                ..Default::default()
            },
            Generation {
                idx: 2,
                profile: None,
                conf_filename: OsString::from("nixos-generation-2.conf"),
                kernel_filename: OsString::from("kernel-2"),
                initrd_filename: OsString::from("initrd-2"),
                ..Default::default()
            },
        ];

        let known_filenames = super::get_required_file_paths(generations.clone()).unwrap();

        for gen in generations {
            assert!(known_filenames.iter().any(|e| e == &gen.conf_filename));
            assert!(known_filenames.iter().any(|e| e == &gen.kernel_filename));
            assert!(known_filenames.iter().any(|e| e == &gen.initrd_filename));
        }
    }
}
