use std::ffi::CStr;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;

use log::{debug, error, info, trace, warn};

use super::version::systemd::SystemdVersion;
use super::version::systemd_boot::SystemdBootVersion;
use crate::util::{self, Generation};
use crate::{Args, Result};

#[derive(Debug)]
pub(crate) enum SystemdBootPlanState<'a> {
    Start, // transition to install or update based on args.install
    Install {
        loader: Option<PathBuf>, // Some(path) if exists
        bootctl: &'a Path,
        esp: &'a Path,
        can_touch_efi_vars: bool,
    },
    Update {
        bootloader_version: Option<SystemdBootVersion>,
        systemd_version: SystemdVersion,
        bootctl: &'a Path,
        esp: &'a Path,
    },
    Prune {
        wanted_generations: Vec<Generation>,
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

pub(crate) fn create_plan<'a>(args: &'a Args, esp: &'a Path) -> Result<SystemdBootPlan<'a>> {
    let mut plan = vec![SystemdBootPlanState::Start];

    // systemd_boot requires the path to bootctl be provided, so it's safe to unwrap (until I make
    // this a subcommand and remove the Option wrapper altogether)
    let bootctl = args.bootctl.as_ref().expect("bootctl was missing");
    let loader = esp.join("loader/loader.conf");

    if args.install {
        let loader = if loader.exists() { Some(loader) } else { None };

        plan.push(SystemdBootPlanState::Install {
            loader,
            bootctl,
            esp,
            can_touch_efi_vars: args.can_touch_efi_vars,
        });
    } else {
        let bootloader_version = SystemdBootVersion::detect_version(bootctl, esp)?;
        let systemd_version = SystemdVersion::detect_version(bootctl)?;

        plan.push(SystemdBootPlanState::Update {
            bootloader_version,
            systemd_version,
            bootctl,
            esp,
        });
    }

    let system_generations = util::all_generations(None)?;
    let wanted_generations =
        super::wanted_generations(system_generations, args.configuration_limit)?;

    // Remove old things from both the generated entries and ESP
    // - Generated entries because we don't need to waste space on copying unused kernels / initrds / entries
    // - ESP so that we don't have unbootable entries
    plan.push(SystemdBootPlanState::Prune {
        wanted_generations: wanted_generations.clone(),
        paths: vec![&args.generated_entries, esp],
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
                console_mode: &args.console_mode,
            });

            break;
        }
    }

    plan.push(SystemdBootPlanState::CopyToEsp {
        generated_entries: &args.generated_entries,
        esp,
    });

    // If there's not enough space for everything, this will error out while copying files, before
    // anything is overwritten via renaming.
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

                    super::remove_old_files(&wanted_generations, path)?;
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
    bootloader_version: Option<SystemdBootVersion>,
    systemd_version: SystemdVersion,
    bootctl: &Path,
    esp: &Path,
) -> Result<()> {
    if let Some(bootloader_version) = bootloader_version {
        if bootloader_version < systemd_version {
            info!(
                "updating systemd-boot from {} to {}",
                bootloader_version.version, systemd_version.version
            );

            let args = &["update", "--path", &esp.display().to_string()];
            debug!("running `{}` with args `{:?}`", &bootctl.display(), &args);
            Command::new(&bootctl).args(args).status()?;
        }
    } else {
        warn!("could not find any previously installed systemd-boot");
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
