use std::ffi::{CStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
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

#[derive(Debug)]
pub(crate) enum SystemdBootPlanState {
    Start, // transition to install or update based on args.install
    Install {
        loader: Option<PathBuf>, // Some(path) if exists
        bootctl: PathBuf,
        esp: PathBuf,
        can_touch_efi_vars: bool,
    },
    Update {
        bootloader_version: Option<SystemdBootVersion>,
        systemd_version: SystemdVersion,
        bootctl: PathBuf,
        esp: PathBuf,
    },
    Prune {
        // transitions to itself for generated_entries, then to esp
        generations: Vec<Generation>,
        path: PathBuf,
    },
    WriteLoader {
        path: PathBuf,
        timeout: Option<usize>,
        index: usize,
        editor: bool,
        console_mode: String,
    },
    // TODO: "Hook" phase here?
    CopyToEsp {
        generated_entries: PathBuf,
        esp: PathBuf,
    },
    Syncfs {
        esp: PathBuf,
    },
    End,
}

type SystemdBootPlan<'a> = Vec<SystemdBootPlanState<'a>>;

pub(crate) fn install(args: Args) -> Result<()> {
    trace!("beginning systemd_boot install process");

    // FIXME: support dry run
    // TODO: make a function (macro_rules! macro?) that accepts the potentially-destructive action and a message to log?
    let dry_run = args.dry_run;
    debug!("dry_run? {}", dry_run);

    let plan = self::create_plan(args)?;

    if dry_run {
        writeln!(std::io::stdout(), "{:#?}", plan)?;
    } else {
        self::consume_plan(plan)?;
    }

    Ok(())
}

pub(crate) fn create_plan(args: Args) -> Result<Vec<SystemdBootPlanState>> {
    let mut plan = vec![SystemdBootPlanState::Start];

    // systemd_boot requires the path to the ESP be provided, so it's safe to unwrap (until I make
    // this a subcommand and remove the Option wrapper altogether)
    let esp = args.esp.unwrap();
    let bootctl = args.bootctl.unwrap();
    let loader = esp.join("loader/loader.conf");

    if args.install {
        let loader = if loader.exists() { Some(loader) } else { None };

        plan.push(SystemdBootPlanState::Install {
            loader,
            bootctl,
            esp: esp.clone(),
            can_touch_efi_vars: args.can_touch_efi_vars,
        });
    } else {
        trace!("updating bootloader");

        let bootloader_version = SystemdBootVersion::detect_version(&bootctl, &esp)?;
        let systemd_version = SystemdVersion::detect_version(&bootctl)?;

        plan.push(SystemdBootPlanState::Update {
            bootloader_version,
            systemd_version,
            bootctl,
            esp: esp.clone(),
        });
    }

    let system_generations = util::all_generations(None)?;
    let wanted_generations =
        self::wanted_generations(system_generations, args.configuration_limit)?;

    // Remove old things from both the generated entries and ESP
    // - Generated entries because we don't need to waste space on copying unused kernels / initrds / entries
    // - ESP so that we don't have unbootable entries

    plan.push(SystemdBootPlanState::Prune {
        generations: wanted_generations.clone(),
        path: args.generated_entries.clone(),
    });
    plan.push(SystemdBootPlanState::Prune {
        generations: wanted_generations.clone(),
        path: esp.clone(),
    });

    // Reverse the iterator because it's more likely that the generation being switched to is
    // "newer", thus will be at the end of the generated list of generations
    debug!("finding default boot entry by comparing store paths");
    for generation in wanted_generations.iter().rev() {
        if fs::canonicalize(&generation.path)? == fs::canonicalize(&args.toplevel)? {
            plan.push(SystemdBootPlanState::WriteLoader {
                path: args.generated_entries.join("loader/loader.conf"),
                timeout: args.timeout,
                index: generation.idx,
                editor: args.editor,
                console_mode: args.console_mode,
            });

            break;
        }
    }

    plan.push(SystemdBootPlanState::CopyToEsp {
        generated_entries: args.generated_entries,
        esp: esp.clone(),
    });

    // If there's not enough space for everything, this will error out while copying files, before
    // anything is overwritten via renaming.
    plan.push(SystemdBootPlanState::Syncfs { esp });

    plan.push(SystemdBootPlanState::End);

    Ok(plan)
}

fn consume_plan(plan: SystemdBootPlan) -> Result<()> {
    use SystemdBootPlanState::*;

    for state in plan {
        match state {
            Start => {
                trace!("started updating / installing");
            }
            Install {
                loader,
                bootctl,
                esp,
                can_touch_efi_vars,
            } => {
                trace!("installing systemd-boot");

                if let Some(loader) = loader {
                    debug!("removing existing loader.conf");
                    fs::remove_file(&loader)?;
                }

                debug!("running `bootctl install`");
                Command::new(&bootctl)
                    .args(&[
                        "install",
                        "--path",
                        &esp.display().to_string(),
                        if !can_touch_efi_vars {
                            "--no-variables"
                        } else {
                            ""
                        },
                    ])
                    .status()?;
            }
            Update {
                bootloader_version,
                systemd_version,
                bootctl,
                esp,
            } => {
                trace!("updating bootloader");

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
            Prune { generations, path } => {
                debug!(
                    "removing old entries / kernels/ initrds from '{}'",
                    &path.display()
                );
                self::remove_old_files(&generations, &path)?;
            }
            WriteLoader {
                path,
                timeout,
                index,
                editor,
                console_mode,
            } => {
                trace!("writing loader.conf for default boot entry");

                // We don't need to check if loader.conf already exists because we are writing it
                // directly to the `generated_entries` directory (where there cannot be one unless
                // manually placed)
                let mut f = File::create(&path)?;
                let contents = self::create_loader_conf(timeout, index, editor, console_mode)?;

                f.write_all(contents.as_bytes())?;
            }
            CopyToEsp {
                generated_entries,
                esp,
            } => {
                debug!("copying everything to the esp");
                util::atomic_tmp_copy(&generated_entries, &esp)?;
            }
            Syncfs { esp } => {
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
            }
            End => {
                trace!("finished updating / installing")
            }
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
