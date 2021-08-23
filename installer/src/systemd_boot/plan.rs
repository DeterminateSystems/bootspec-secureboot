use std::ffi::CStr;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;

use log::{debug, error, info, trace};

use super::version::systemd::SystemdVersion;
use super::version::systemd_boot::SystemdBootVersion;
use crate::util::{self, Generation};
use crate::{Args, Result};

#[derive(Debug, PartialEq)]
pub(crate) enum SystemdBootPlanState<'a> {
    Start, // transition to install or update based on args.install
    Install {
        loader: Option<PathBuf>, // Some(path) if exists
        bootctl: &'a Path,
        esp: &'a Path,
        can_touch_efi_vars: bool,
    },
    Update {
        bootloader_version: SystemdBootVersion,
        systemd_version: SystemdVersion,
        bootctl: &'a Path,
        esp: &'a Path,
    },
    Prune {
        wanted_generations: &'a [Generation],
        paths: Vec<&'a Path>,
    },
    WriteLoader {
        path: PathBuf,
        timeout: Option<usize>,
        index: usize,
        editor: bool,
        console_mode: &'a str,
    },
    // TODO: "Hook" phase here?
    CopyToEsp {
        generated_entries: &'a Path,
        esp: &'a Path,
    },
    Syncfs {
        esp: &'a Path,
    },
    End,
}

type SystemdBootPlan<'a> = Vec<SystemdBootPlanState<'a>>;

pub(crate) fn create_plan<'a>(
    args: &'a Args,
    bootctl: &'a Path,
    esp: &'a Path,
    bootloader_version: Option<SystemdBootVersion>,
    systemd_version: SystemdVersion,
    wanted_generations: &'a [Generation],
    default_generation: &'a Generation,
) -> Result<SystemdBootPlan<'a>> {
    let mut plan = vec![SystemdBootPlanState::Start];

    if args.install {
        let loader = esp.join("loader/loader.conf");

        plan.push(SystemdBootPlanState::Install {
            loader: if loader.exists() { Some(loader) } else { None },
            bootctl,
            esp,
            can_touch_efi_vars: args.can_touch_efi_vars,
        });
    } else {
        // We require a bootloader_version when updating (the default operation), so this is safe to unwrap.
        let bootloader_version =
            bootloader_version.expect("bootloader version was None, but we're updating");

        plan.push(SystemdBootPlanState::Update {
            bootloader_version,
            systemd_version,
            bootctl,
            esp,
        });
    }

    // Remove old things from both the generated entries and ESP
    // - Generated entries because we don't need to waste space on copying unused kernels / initrds / entries
    // - ESP so that we don't have unbootable entries
    plan.push(SystemdBootPlanState::Prune {
        wanted_generations,
        paths: vec![&args.generated_entries, esp],
    });

    plan.push(SystemdBootPlanState::WriteLoader {
        path: args.generated_entries.join("loader/loader.conf"),
        timeout: args.timeout,
        index: default_generation.idx,
        editor: args.editor,
        console_mode: &args.console_mode,
    });

    plan.push(SystemdBootPlanState::CopyToEsp {
        generated_entries: &args.generated_entries,
        esp,
    });

    plan.push(SystemdBootPlanState::Syncfs { esp });

    plan.push(SystemdBootPlanState::End);

    Ok(plan)
}

pub(crate) fn consume_plan(plan: SystemdBootPlan) -> Result<()> {
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
                self::run_install(loader, bootctl, esp, can_touch_efi_vars)?;
            }
            Update {
                bootloader_version,
                systemd_version,
                bootctl,
                esp,
            } => {
                trace!("updating systemd-boot");
                self::run_update(bootloader_version, systemd_version, bootctl, esp)?;
            }
            Prune {
                wanted_generations,
                paths,
            } => {
                trace!("pruning paths: {:?}", &paths);

                for path in paths {
                    debug!(
                        "removing old entries / kernels / initrds from '{}'",
                        &path.display()
                    );

                    super::remove_old_files(wanted_generations, path)?;
                }
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
                let contents = super::create_loader_conf(timeout, index, editor, console_mode)?;

                f.write_all(contents.as_bytes())?;
            }
            CopyToEsp {
                generated_entries,
                esp,
            } => {
                trace!("copying everything to the esp");

                // If there's not enough space for everything, this will error out while copying files, before
                // anything is overwritten via renaming.
                util::atomic_tmp_copy(generated_entries, esp)?;
            }
            Syncfs { esp } => {
                trace!("attempting to syncfs(2) the esp");
                self::syncfs(esp)?;
            }
            End => {
                trace!("finished updating / installing")
            }
        }
    }

    Ok(())
}

fn run_install(
    loader: Option<PathBuf>,
    bootctl: &Path,
    esp: &Path,
    can_touch_efi_vars: bool,
) -> Result<()> {
    if let Some(loader) = loader {
        debug!("removing existing loader.conf");
        fs::remove_file(&loader)?;
    }

    let args = &[
        "install",
        "--path",
        &esp.display().to_string(),
        if !can_touch_efi_vars {
            "--no-variables"
        } else {
            ""
        },
    ];
    debug!("running `{}` with args `{:?}`", &bootctl.display(), &args);
    Command::new(&bootctl).args(args).status()?;

    Ok(())
}

fn run_update(
    bootloader_version: SystemdBootVersion,
    systemd_version: SystemdVersion,
    bootctl: &Path,
    esp: &Path,
) -> Result<()> {
    if bootloader_version < systemd_version {
        info!(
            "updating systemd-boot from {} to {}",
            bootloader_version.version, systemd_version.version
        );

        let args = &["update", "--path", &esp.display().to_string()];
        debug!("running `{}` with args `{:?}`", &bootctl.display(), &args);
        Command::new(&bootctl).args(args).status()?;
    }

    Ok(())
}

fn syncfs(esp: &Path) -> Result<()> {
    let f = File::open(&esp)?;
    let fd = f.as_raw_fd();

    // SAFETY: idk
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn scaffold(install: bool) -> (Args, Vec<Generation>, Generation) {
        let args = Args {
            toplevel: PathBuf::from("toplevel"),
            dry_run: false,
            generated_entries: PathBuf::from("generated_entries"),
            timeout: Some(1),
            console_mode: String::from("max"),
            configuration_limit: Some(1),
            editor: false,
            verbosity: 0,
            install,
            esp: vec![PathBuf::from("esp")],
            can_touch_efi_vars: false,
            bootctl: Some(PathBuf::from("bootctl")),
        };
        let system_generations = vec![
            Generation {
                idx: 1,
                profile: None,
                path: PathBuf::from("1"),
                conf_filename: OsString::from("nixos-generation-1.conf"),
                kernel_filename: OsString::from("abcd-linux-5.12.9-bzImage.efi"),
                initrd_filename: OsString::from("abcd-initrd-linux-5.12.9-initrd.efi"),
            },
            Generation {
                idx: 2,
                profile: None,
                path: PathBuf::from("2"),
                conf_filename: OsString::from("nixos-generation-2.conf"),
                kernel_filename: OsString::from("abcd-linux-5.12.9-bzImage.efi"),
                initrd_filename: OsString::from("abcd-initrd-linux-5.12.9-initrd.efi"),
            },
        ];
        let wanted_generations =
            util::wanted_generations(system_generations, args.configuration_limit);
        let default_generation = Generation {
            idx: 2,
            profile: None,
            path: PathBuf::from("2"),
            conf_filename: OsString::from("nixos-generation-2.conf"),
            kernel_filename: OsString::from("abcd-linux-5.12.9-bzImage.efi"),
            initrd_filename: OsString::from("abcd-initrd-linux-5.12.9-initrd.efi"),
        };

        (args, wanted_generations, default_generation)
    }

    #[test]
    fn test_update_plan() {
        let (args, wanted_generations, default_generation) = scaffold(false);
        let systemd_version = SystemdVersion::new(247);
        let bootloader_version = SystemdBootVersion::new(246);
        let bootctl = args.bootctl.as_ref().unwrap();
        let esp = &args.esp[0];

        let plan = create_plan(
            &args,
            bootctl,
            esp,
            Some(bootloader_version),
            systemd_version,
            &wanted_generations,
            &default_generation,
        )
        .unwrap();
        dbg!(&plan);
        let mut iter = plan.into_iter();

        assert_eq!(iter.next().unwrap(), SystemdBootPlanState::Start);
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::Update {
                bootloader_version: SystemdBootVersion::new(246),
                systemd_version: SystemdVersion::new(247),
                bootctl,
                esp,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::Prune {
                wanted_generations: &wanted_generations,
                paths: vec![&args.generated_entries, esp],
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::WriteLoader {
                path: args.generated_entries.join("loader/loader.conf"),
                timeout: args.timeout,
                index: default_generation.idx,
                editor: args.editor,
                console_mode: &args.console_mode,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::CopyToEsp {
                generated_entries: &args.generated_entries,
                esp,
            }
        );
        assert_eq!(iter.next().unwrap(), SystemdBootPlanState::Syncfs { esp });
        assert_eq!(iter.next().unwrap(), SystemdBootPlanState::End);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_install_plan() {
        let (args, wanted_generations, default_generation) = scaffold(true);
        let systemd_version = SystemdVersion::new(247);
        let bootctl = args.bootctl.as_ref().unwrap();
        let esp = &args.esp[0];

        let plan = create_plan(
            &args,
            bootctl,
            esp,
            None,
            systemd_version,
            &wanted_generations,
            &default_generation,
        )
        .unwrap();
        dbg!(&plan);
        let mut iter = plan.into_iter();

        assert_eq!(iter.next().unwrap(), SystemdBootPlanState::Start);
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::Install {
                loader: None,
                bootctl,
                esp,
                can_touch_efi_vars: args.can_touch_efi_vars,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::Prune {
                wanted_generations: &wanted_generations,
                paths: vec![&args.generated_entries, esp],
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::WriteLoader {
                path: args.generated_entries.join("loader/loader.conf"),
                timeout: args.timeout,
                index: default_generation.idx,
                editor: args.editor,
                console_mode: &args.console_mode,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SystemdBootPlanState::CopyToEsp {
                generated_entries: &args.generated_entries,
                esp,
            }
        );
        assert_eq!(iter.next().unwrap(), SystemdBootPlanState::Syncfs { esp });
        assert_eq!(iter.next().unwrap(), SystemdBootPlanState::End);
        assert_eq!(iter.next(), None);
    }
}
