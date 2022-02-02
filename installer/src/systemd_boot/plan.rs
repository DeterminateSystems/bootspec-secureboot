use std::ffi::{CStr, OsStr};
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crc::{Crc, CRC_32_ISCSI};
use log::{debug, error, info, trace, warn};

use super::version::systemd::SystemdVersion;
use crate::files::{FileToReplace, IdentifiedFiles};
use crate::secure_boot::SigningInfo;
use crate::util::{self, Generation};
use crate::{Args, Result};

const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

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
        bootctl: &'a Path,
        esp: &'a Path,
    },
    PruneFiles {
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
    ReplaceFiles {
        signing_info: &'a Option<SigningInfo>,
        to_replace: Vec<FileToReplace>,
    },
    SignFiles {
        signing_info: &'a SigningInfo,
        to_sign: Vec<PathBuf>,
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

pub(crate) struct PlanArgs<'a> {
    pub args: &'a Args,
    pub bootctl: &'a Path,
    pub esp: &'a Path,
    pub wanted_generations: &'a [Generation],
    pub default_generation: &'a Generation,
    pub identified_files: IdentifiedFiles,
}

pub(crate) fn create_plan(plan_args: PlanArgs) -> Result<SystemdBootPlan> {
    let args = plan_args.args;
    let bootctl = plan_args.bootctl;
    let esp = plan_args.esp;
    let wanted_generations = plan_args.wanted_generations;
    let default_generation = plan_args.default_generation;
    let identified_files = plan_args.identified_files;

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
        plan.push(SystemdBootPlanState::Update { bootctl, esp });
    }

    if let Some(signing_info) = &args.signing_info {
        let mut to_sign = vec![];
        to_sign.push(esp.join("EFI/systemd/systemd-bootx64.efi"));
        to_sign.push(esp.join("EFI/BOOT/BOOTX64.EFI"));
        to_sign.extend(identified_files.to_sign);

        plan.push(SystemdBootPlanState::SignFiles {
            signing_info,
            to_sign,
        });
    }

    // Remove old things from both the generated entries and ESP
    // - Generated entries because we don't need to waste space on copying unused kernels / initrds / entries
    // - ESP so that we don't have unbootable entries
    plan.push(SystemdBootPlanState::PruneFiles {
        wanted_generations,
        paths: vec![&args.generated_entries, esp],
    });

    plan.push(SystemdBootPlanState::ReplaceFiles {
        signing_info: &args.signing_info,
        to_replace: identified_files.to_replace,
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
            Update { bootctl, esp } => {
                trace!("updating systemd-boot");
                self::run_update(bootctl, esp)?;
            }
            SignFiles {
                signing_info,
                to_sign,
            } => {
                trace!("signing efi files");

                for file in to_sign {
                    signing_info.sign_file(&file)?;
                }
            }
            PruneFiles {
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
            ReplaceFiles {
                signing_info,
                to_replace,
            } => {
                trace!("replacing existing files in esp");

                for file in to_replace {
                    self::replace_file(&file, signing_info)?;
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
                fs::remove_dir_all(&generated_entries)?;
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

    let mut args = vec![
        String::from("install"),
        String::from("--path"),
        esp.display().to_string(),
    ];
    if !can_touch_efi_vars {
        args.push(String::from("--no-variables"));
    }
    debug!("running `{}` with args `{:?}`", &bootctl.display(), &args);
    let status = Command::new(&bootctl).args(&args).status()?;

    if !status.success() {
        return Err(format!(
            "failed to run `{}` with args `{:?}`",
            &bootctl.display(),
            &args
        )
        .into());
    }

    Ok(())
}

fn run_update(bootctl: &Path, esp: &Path) -> Result<()> {
    let systemd_version = SystemdVersion::detect_version(bootctl)?;
    info!("updating systemd-boot to {}", systemd_version.version);

    let args = &["update", "--path", &esp.display().to_string()];
    debug!("running `{}` with args `{:?}`", &bootctl.display(), &args);
    let status = Command::new(&bootctl).args(args).status()?;

    if !status.success() {
        return Err(format!(
            "failed to run `{}` with args `{:?}`",
            &bootctl.display(),
            &args
        )
        .into());
    }

    Ok(())
}

fn replace_file(file: &FileToReplace, signing_info: &Option<SigningInfo>) -> Result<()> {
    let generated_loc = &file.generated_loc;
    let esp_loc = &file.esp_loc;

    let (hash_a, hash_b) = if signing_info.is_some()
        && generated_loc.extension() == Some(OsStr::new("efi"))
    {
        let signing_info = signing_info.as_ref().unwrap();

        // If the signed file in the generated location doesn't validate, something went
        // horribly wrong and this error *should* be bubbled up.
        signing_info.verify_file(generated_loc)?;

        // However, if the signed file in the ESP location doesn't validate, we will be
        // replacing it with the generated file; just warn the user.
        if let Err(e) = signing_info.verify_file(esp_loc) {
            warn!("{}", e);
        }

        let tmp_dir = std::env::temp_dir();
        let generated_tmp = tmp_dir.join("generated");
        let esp_tmp = tmp_dir.join("esp");

        fs::copy(&generated_loc, &generated_tmp)?;
        fs::copy(&esp_loc, &esp_tmp)?;

        let sbattach = env!("PATCHED_SBATTACH_BINARY");
        let args = &["--remove", &generated_tmp.display().to_string()];
        debug!("running `{}` with args `{:?}`", &sbattach, &args);
        let status = Command::new(sbattach)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(format!(
                "failed to remove signature from '{}'",
                generated_tmp.display()
            )
            .into());
        }

        let args = &["--remove", &esp_tmp.display().to_string()];
        debug!("running `{}` with args `{:?}`", &sbattach, &args);
        let status = Command::new(sbattach)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(format!("failed to remove signature from '{}'", esp_tmp.display()).into());
        }

        let hash_a = CASTAGNOLI.checksum(&fs::read(&generated_tmp)?);
        let hash_b = CASTAGNOLI.checksum(&fs::read(&esp_tmp)?);

        fs::remove_file(&generated_tmp)?;
        fs::remove_file(&esp_tmp)?;

        (hash_a, hash_b)
    } else {
        let hash_a = CASTAGNOLI.checksum(&fs::read(&generated_loc)?);
        let hash_b = CASTAGNOLI.checksum(&fs::read(&esp_loc)?);

        (hash_a, hash_b)
    };

    if hash_a == hash_b {
        debug!(
            "{} and {} are the same file",
            esp_loc.display(),
            generated_loc.display()
        );
        fs::remove_file(&generated_loc)?;
    } else {
        warn!(
            "{} is different from {} and will be replaced",
            esp_loc.display(),
            generated_loc.display(),
        );
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

    fn scaffold(
        install: bool,
        signing_info: Option<SigningInfo>,
    ) -> (Args, Vec<Generation>, Generation, IdentifiedFiles) {
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
            unified_efi: false,
            signing_info,
        };
        let system_generations = vec![
            Generation {
                idx: 1,
                profile: None,
                path: PathBuf::from("1"),
                required_filenames: vec![
                    OsString::from("nixos-generation-1.conf"),
                    OsString::from("abcd-linux-5.12.9-bzImage.efi"),
                    OsString::from("abcd-initrd-linux-5.12.9-initrd.efi"),
                ],
            },
            Generation {
                idx: 2,
                profile: None,
                path: PathBuf::from("2"),
                required_filenames: vec![
                    OsString::from("nixos-generation-2.conf"),
                    OsString::from("abcd-linux-5.12.9-bzImage.efi"),
                    OsString::from("abcd-initrd-linux-5.12.9-initrd.efi"),
                ],
            },
        ];
        let wanted_generations =
            util::wanted_generations(system_generations, args.configuration_limit);
        let default_generation = Generation {
            idx: 2,
            profile: None,
            path: PathBuf::from("2"),
            required_filenames: vec![
                OsString::from("nixos-generation-2.conf"),
                OsString::from("abcd-linux-5.12.9-bzImage.efi"),
                OsString::from("abcd-initrd-linux-5.12.9-initrd.efi"),
            ],
        };
        let identified_files = IdentifiedFiles {
            to_sign: vec![
                PathBuf::from("abcd-linux-5.12.9-bzImage.efi"),
                PathBuf::from("abcd-initrd-linux-5.12.9-initrd.efi"),
            ],
            to_replace: vec![],
        };

        (
            args,
            wanted_generations,
            default_generation,
            identified_files,
        )
    }

    #[test]
    fn test_update_plan() {
        let (args, wanted_generations, default_generation, identified_files) =
            scaffold(false, None);
        let bootctl = args.bootctl.as_ref().unwrap();
        let esp = &args.esp[0];
        let plan_args = PlanArgs {
            args: &args,
            bootctl,
            esp,
            wanted_generations: &wanted_generations,
            default_generation: &default_generation,
            identified_files,
        };

        let plan = create_plan(plan_args).unwrap();
        dbg!(&plan);

        assert_eq!(
            plan,
            vec![
                SystemdBootPlanState::Start,
                SystemdBootPlanState::Update { bootctl, esp },
                SystemdBootPlanState::PruneFiles {
                    wanted_generations: &wanted_generations,
                    paths: vec![&args.generated_entries, esp],
                },
                SystemdBootPlanState::ReplaceFiles {
                    signing_info: &None,
                    to_replace: vec![],
                },
                SystemdBootPlanState::WriteLoader {
                    path: args.generated_entries.join("loader/loader.conf"),
                    timeout: args.timeout,
                    index: default_generation.idx,
                    editor: args.editor,
                    console_mode: &args.console_mode,
                },
                SystemdBootPlanState::CopyToEsp {
                    generated_entries: &args.generated_entries,
                    esp,
                },
                SystemdBootPlanState::Syncfs { esp },
                SystemdBootPlanState::End
            ]
        );
    }

    #[test]
    fn test_install_plan() {
        let (args, wanted_generations, default_generation, identified_files) = scaffold(true, None);
        let bootctl = args.bootctl.as_ref().unwrap();
        let esp = &args.esp[0];
        let plan_args = PlanArgs {
            args: &args,
            bootctl,
            esp,
            wanted_generations: &wanted_generations,
            default_generation: &default_generation,
            identified_files,
        };

        let plan = create_plan(plan_args).unwrap();
        dbg!(&plan);

        assert_eq!(
            plan,
            vec![
                SystemdBootPlanState::Start,
                SystemdBootPlanState::Install {
                    loader: None,
                    bootctl,
                    esp,
                    can_touch_efi_vars: args.can_touch_efi_vars,
                },
                SystemdBootPlanState::PruneFiles {
                    wanted_generations: &wanted_generations,
                    paths: vec![&args.generated_entries, esp],
                },
                SystemdBootPlanState::ReplaceFiles {
                    signing_info: &None,
                    to_replace: vec![],
                },
                SystemdBootPlanState::WriteLoader {
                    path: args.generated_entries.join("loader/loader.conf"),
                    timeout: args.timeout,
                    index: default_generation.idx,
                    editor: args.editor,
                    console_mode: &args.console_mode,
                },
                SystemdBootPlanState::CopyToEsp {
                    generated_entries: &args.generated_entries,
                    esp,
                },
                SystemdBootPlanState::Syncfs { esp },
                SystemdBootPlanState::End
            ]
        );
    }

    #[test]
    fn test_sign_plan() {
        let signing_info = SigningInfo {
            signing_key: PathBuf::from("db.key"),
            signing_cert: PathBuf::from("db.crt"),
            sbsign: PathBuf::from("sbsign"),
            sbverify: PathBuf::from("sbverify"),
        };
        let (args, wanted_generations, default_generation, identified_files) =
            scaffold(false, Some(signing_info));
        let bootctl = args.bootctl.as_ref().unwrap();
        let esp = &args.esp[0];
        let plan_args = PlanArgs {
            args: &args,
            bootctl,
            esp,
            wanted_generations: &wanted_generations,
            default_generation: &default_generation,
            identified_files: identified_files.clone(),
        };

        let plan = create_plan(plan_args).unwrap();
        let mut to_sign = vec![];
        to_sign.push(esp.join("EFI/systemd/systemd-bootx64.efi"));
        to_sign.push(esp.join("EFI/BOOT/BOOTX64.EFI"));
        to_sign.extend(identified_files.to_sign);

        assert_eq!(
            plan,
            vec![
                SystemdBootPlanState::Start,
                SystemdBootPlanState::Update { bootctl, esp },
                SystemdBootPlanState::SignFiles {
                    signing_info: args.signing_info.as_ref().unwrap(),
                    to_sign
                },
                SystemdBootPlanState::PruneFiles {
                    wanted_generations: &wanted_generations,
                    paths: vec![&args.generated_entries, esp],
                },
                SystemdBootPlanState::ReplaceFiles {
                    signing_info: &args.signing_info,
                    to_replace: vec![],
                },
                SystemdBootPlanState::WriteLoader {
                    path: args.generated_entries.join("loader/loader.conf"),
                    timeout: args.timeout,
                    index: default_generation.idx,
                    editor: args.editor,
                    console_mode: &args.console_mode,
                },
                SystemdBootPlanState::CopyToEsp {
                    generated_entries: &args.generated_entries,
                    esp,
                },
                SystemdBootPlanState::Syncfs { esp },
                SystemdBootPlanState::End
            ]
        );
    }
}
