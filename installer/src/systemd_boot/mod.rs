use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use log::{debug, trace, warn};
use regex::Regex;

use crate::files::IdentifiedFiles;
use crate::systemd_boot::plan::PlanArgs;
use crate::util::{self, Generation};
use crate::{Args, Result};

mod plan;
mod version;

lazy_static::lazy_static! {
    static ref ENTRY_RE: Regex = Regex::new("nixos-(?:(?P<profile>[^-]+)-)?generation-(?P<generation>\\d+).conf").unwrap();
}

pub(crate) fn install(args: Args) -> Result<()> {
    trace!("beginning systemd-boot install process");
    debug!("dry_run? {}", args.dry_run);

    if args.esp.is_empty() {
        return Err("No ESP(s) specified; exiting.".into());
    }

    let esps = &args.esp;
    let bootctl = args.bootctl.as_ref().expect("bootctl was missing");
    let system_generations = util::all_generations(None, args.unified_efi)?;
    let wanted_generations = util::wanted_generations(system_generations, args.configuration_limit);
    let default_generation = wanted_generations
        .iter()
        .rev()
        .find(|generation| {
            fs::canonicalize(&generation.path).ok() == fs::canonicalize(&args.toplevel).ok()
        })
        .ok_or("couldn't find generation that corresponds to the provided toplevel")?;

    for esp in esps {
        let identified_files = IdentifiedFiles::new(&args.generated_entries, esp)?;

        let plan_args = PlanArgs {
            args: &args,
            bootctl,
            esp,
            wanted_generations: &wanted_generations,
            default_generation,
            identified_files,
        };

        let plan = plan::create_plan(plan_args)?;

        if args.dry_run {
            writeln!(std::io::stdout(), "{:#?}", plan)?;
        } else {
            fs::create_dir_all(esp.join("EFI/nixos"))?;
            fs::create_dir_all(esp.join("loader/entries"))?;

            plan::consume_plan(plan)?;
        }
    }

    Ok(())
}

fn create_loader_conf(
    timeout: Option<usize>,
    idx: usize,
    editor: bool,
    console_mode: &str,
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

fn get_required_filenames(generations: Vec<Generation>) -> Vec<OsString> {
    let mut required_filenames = Vec::new();

    for generation in generations {
        required_filenames.extend(generation.required_filenames);
    }

    required_filenames
}

// TODO: split into different binary / subcommand?
fn remove_old_files(generations: &[Generation], path: &Path) -> Result<()> {
    trace!("removing old files");

    let efi_nixos = path.join("EFI/nixos");
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

    debug!("calculating required filenames");
    let required_filenames = self::get_required_filenames(generations.to_vec());

    debug!("removing old entries");
    for entry in fs::read_dir(loader_entries)? {
        let f = entry?.path();
        let name = f.file_name().ok_or("filename terminated in ..")?;

        // Don't want to delete user's custom boot entries
        if !ENTRY_RE.is_match(&name.to_string_lossy()) {
            continue;
        }

        if !required_filenames.iter().any(|e| e == name) {
            fs::remove_file(f)?;
        }
    }

    debug!("removing old kernels / initrds");
    for entry in fs::read_dir(efi_nixos)? {
        let f = entry?.path();
        let name = f.file_name().ok_or("filename terminated in ..")?;

        if !required_filenames.iter().any(|e| e == name) {
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
            super::create_loader_conf(Some(1), 125, true, "max").unwrap(),
            r#"timeout 1
default nixos-generation-125.conf
console-mode max
"#
        );
        assert_eq!(
            super::create_loader_conf(Some(2), 126, false, "max").unwrap(),
            r#"timeout 2
default nixos-generation-126.conf
editor 0
console-mode max
"#
        );
    }

    #[test]
    fn test_get_known_filenames() {
        let generations = vec![
            Generation {
                idx: 1,
                profile: None,
                required_filenames: vec![
                    OsString::from("nixos-generation-1.conf"),
                    OsString::from("kernel-1"),
                    OsString::from("initrd-1"),
                ],
                ..Default::default()
            },
            Generation {
                idx: 2,
                profile: None,
                required_filenames: vec![
                    OsString::from("nixos-generation-2.conf"),
                    OsString::from("kernel-2"),
                    OsString::from("initrd-2"),
                ],
                ..Default::default()
            },
        ];

        let required_filenames = super::get_required_filenames(generations.clone());

        for gen in generations {
            assert!(gen
                .required_filenames
                .iter()
                .any(|e| required_filenames.contains(e)));
            assert!(gen
                .required_filenames
                .iter()
                .any(|e| required_filenames.contains(e)));
            assert!(gen
                .required_filenames
                .iter()
                .any(|e| required_filenames.contains(e)));
        }
    }
}
