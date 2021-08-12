use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Result;

// TODO: docstrings for these functions

#[derive(Debug)]
pub struct Generation {
    pub idx: usize,
    pub profile: Option<String>,
    pub path: PathBuf,
}

pub fn all_generations(profile: Option<String>) -> Result<Vec<Generation>> {
    let profile_path = profile_path(&profile);
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

        generations.push(Generation {
            idx: generation.parse()?,
            profile: profile.clone(),
            path: PathBuf::from(format!("{}-{}-link", profile_path, generation)),
        });
    }

    Ok(generations)
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

/// Copies `source` to a temporary directory near `dest` recursively, and then atomically moves all the files to the desired location.
pub fn atomic_tmp_copy(source: &Path, dest: &Path) -> Result<()> {
    let tmp_dest = dest.join("tmp");

    if tmp_dest.exists() {
        fs::remove_dir_all(&tmp_dest)?;
    }

    self::copy_impl(&source, &tmp_dest, None, false)?;
    self::copy_impl(&tmp_dest, &dest, None, true)?;
    fs::remove_dir_all(&tmp_dest)?;

    Ok(())
}

// https://github.com/mdunsmuir/copy_dir/blob/071bab19cd716825375e70644c080c36a58863a1/src/lib.rs#L118
// Original work Copyright (c) 2016 Michael Dunsmuir
// Modified work Copyright (c) 2019 Cole Helbling
// Modified work Copyright (c) 2021 Determinate Systems, Inc.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
fn copy_impl<P, Q>(source: &P, dest: &Q, mut root: Option<(u64, u64)>, rename: bool) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    fn uid(path: &Path) -> Result<(u64, u64)> {
        let metadata = path.metadata()?;
        Ok((metadata.dev(), metadata.ino()))
    }

    let source = source.as_ref();
    let dest = dest.as_ref();
    let id = uid(source)?;
    let meta = source.metadata()?;

    if meta.is_file() {
        if fs::metadata(&dest).is_err() {
            self::create_dirs_to_file(&dest)?;
        }

        if rename {
            fs::rename(source, dest)?;
        } else {
            fs::copy(source, dest)?;
        }
    } else if meta.is_dir() {
        if let Some(root) = root {
            if root == id {
                return Err("source is destination".into());
            }
        }

        fs::create_dir_all(&dest)?;

        if root.is_none() {
            root = Some(uid(dest)?);
        }

        for entry in fs::read_dir(source)? {
            let entry = entry?.path();
            let name = entry
                .file_name()
                .ok_or("Entry did not contain valid filename")?;
            self::copy_impl(&entry, &dest.join(name), root, rename)?;
        }

        fs::set_permissions(dest, meta.permissions())?;
    } else {
        // not file or dir (probably -> doesn't exist)
    }

    Ok(())
}
