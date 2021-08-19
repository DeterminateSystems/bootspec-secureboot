use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use log::{debug, trace, warn};
use regex::Regex;

use crate::util::Generation;
use crate::{Args, Result};

mod plan;
mod version;

lazy_static::lazy_static! {
    static ref ENTRY_RE: Regex = Regex::new("nixos-(?:(?P<profile>[^-]+)-)?generation-(?P<generation>\\d+).conf").unwrap();
}

pub(crate) fn install(args: Args) -> Result<()> {
    trace!("beginning systemd_boot install process");

    // FIXME: support dry run
    // TODO: make a function (macro_rules! macro?) that accepts the potentially-destructive action and a message to log?
    let dry_run = args.dry_run;
    debug!("dry_run? {}", dry_run);

    if args.esp.is_empty() {
        return Err("No ESP(s) specified; exiting.".into());
    }

    let esps = &args.esp;
    for esp in esps {
        let plan = plan::create_plan(&args, esp)?;

        if dry_run {
            writeln!(std::io::stdout(), "{:#?}", plan)?;
        } else {
            plan::consume_plan(plan)?;
        }
    }

    Ok(())
}

pub(crate) fn create_loader_conf(
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

pub(crate) fn wanted_generations(
    generations: Vec<Generation>,
    configuration_limit: Option<usize>,
) -> Vec<Generation> {
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

    generations
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
            let ret_generations = super::wanted_generations(generations.clone(), None);
            assert_eq!(ret_generations.len(), 2);
            assert_eq!(ret_generations[0], generations[0]);
            assert_eq!(ret_generations[1], generations[1]);

            let ret_generations = super::wanted_generations(generations.clone(), Some(1));
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
