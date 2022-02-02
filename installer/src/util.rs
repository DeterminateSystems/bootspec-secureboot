use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use log::{debug, trace};
use regex::Regex;

use crate::Result;

// TODO: docstrings for these functions

// TODO: shared crate that has all these constant-like things in it so they don't get out of sync?
lazy_static::lazy_static! {
    static ref GENERATION_RE: Regex = Regex::new("/(?P<profile>[^-]+)-(?P<generation>\\d+)-link").unwrap();
}

const STORE_PATH_PREFIX: &str = "/nix/store/";
const STORE_HASH_LEN: usize = 32;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Generation {
    pub idx: usize,
    pub profile: Option<String>,
    pub path: PathBuf,
    pub required_filenames: Vec<OsString>,
}

pub fn wanted_generations(
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

pub fn all_generations(profile: Option<String>, unified: bool) -> Result<Vec<Generation>> {
    let mut generations = Vec::new();
    let profile_path = self::profile_path(&profile);
    let pat = format!("{}-*-link", profile_path);

    for entry in glob::glob(&pat)? {
        let path = entry?;
        let s = path.display().to_string();
        let idx = GENERATION_RE
            .captures(&s)
            .and_then(|c| c.name("generation"))
            .expect("couldn't find generation")
            .as_str()
            .parse::<usize>()?;

        let conf_filename = if let Some(profile) = &profile {
            format!("nixos-{}-generation-{}.conf", profile, idx)
        } else {
            format!("nixos-generation-{}.conf", idx)
        };

        let required_filenames = if unified {
            let path = fs::canonicalize(&path)?;
            let filename = format!(
                "{}.efi",
                &path.display().to_string().replace(STORE_PATH_PREFIX, "")[..STORE_HASH_LEN]
            );

            vec![filename.into(), conf_filename.into()]
        } else {
            let kernel_path = fs::canonicalize(path.join("kernel"))?;
            let kernel_filename = self::store_path_to_filename(kernel_path)?;
            let initrd_path = fs::canonicalize(path.join("initrd"))?;
            let initrd_filename = self::store_path_to_filename(initrd_path)?;

            vec![kernel_filename, initrd_filename, conf_filename.into()]
        };

        generations.push(Generation {
            idx,
            profile: profile.clone(),
            path,
            required_filenames,
        });
    }

    generations.sort_by(|a, b| a.idx.cmp(&b.idx));

    Ok(generations)
}

pub fn store_path_to_filename(path: PathBuf) -> Result<OsString> {
    let s = path.to_string_lossy();

    if !s.starts_with(STORE_PATH_PREFIX) {
        return Err("provided path wasn't a Nix store path".into());
    }

    let s = s.replace(STORE_PATH_PREFIX, "").replace("/", "-") + ".efi";

    Ok(s.into())
}

pub fn profile_path(profile: &Option<String>) -> String {
    if let Some(ref profile) = profile {
        format!("/nix/var/nix/profiles/system-profiles/{}", profile)
    } else {
        String::from("/nix/var/nix/profiles/system")
    }
}

/// A light wrapper around [`fs::create_dir_all`] that creates all directories
/// to allow the specified `file` to be created.
///
/// [`fs::create_dir_all`]: https://doc.rust-lang.org/std/fs/fn.create_dir_all.html
pub fn create_dirs_to_file<P>(path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    if path.exists() {
        return Ok(());
    }

    let dir = path
        .parent()
        .ok_or(format!("Path '{}' had no parent", path.display()))?;

    fs::create_dir_all(dir)?;

    Ok(())
}

/// Copies `source` to `dest` with a ".tmp" file extension, and then atomically moves it to the desired location.
pub fn atomic_tmp_copy_file(source: &Path, dest: &Path) -> Result<()> {
    let tmp_dest = dest.with_extension("tmp");

    if tmp_dest.exists() {
        fs::remove_file(&tmp_dest)?;
    }

    self::create_dirs_to_file(dest)?;
    fs::copy(source, &tmp_dest)?;
    fs::rename(tmp_dest, dest)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Generation;
    use std::ffi::OsString;

    #[test]
    fn test_wanted_generations() {
        let gens = [
            vec![
                Generation {
                    idx: 1,
                    profile: None,
                    required_filenames: vec![OsString::from("nixos-generation-1.conf")],
                    ..Default::default()
                },
                Generation {
                    idx: 2,
                    profile: None,
                    required_filenames: vec![OsString::from("nixos-generation-2.conf")],
                    ..Default::default()
                },
            ],
            vec![
                Generation {
                    idx: 1,
                    profile: Some(String::from("test")),
                    required_filenames: vec![OsString::from("nixos-generation-1.conf")],
                    ..Default::default()
                },
                Generation {
                    idx: 2,
                    profile: Some(String::from("test")),
                    required_filenames: vec![OsString::from("nixos-generation-2.conf")],
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
}
