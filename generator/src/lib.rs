use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use bootspec::{BootJson, JSON_FILENAME};
use regex::Regex;

pub mod bootable;
pub mod grub;
pub mod systemd_boot;

#[derive(Debug, Default)]
pub struct Generation {
    pub index: usize,
    pub profile: Option<String>,
    pub bootspec: BootJson,
}

pub type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

lazy_static::lazy_static! {
    static ref SYSTEM_RE: Regex = Regex::new("/profiles/system-(?P<generation>\\d+)-link").unwrap();
    static ref PROFILE_RE: Regex = Regex::new("/system-profiles/(?P<profile>[^-]+)-(?P<generation>\\d+)-link").unwrap();
}

pub fn get_json(tempdir: &Path, generation_path: PathBuf) -> Result<BootJson> {
    let json_path = generation_path.join(JSON_FILENAME);

    let mut json: Option<BootJson> = None;
    if json_path.exists() {
        if let Ok(cont) = fs::read_to_string(&json_path) {
            if let Ok(parsed) = serde_json::from_str(&cont) {
                json = Some(parsed)
            }
        }
    }

    if json.is_none() {
        let dest = tempdir.join("synthesis");

        // Time to synthesize
        synthesize::synthesize_schema_from_generation(&generation_path, &dest)?;
        let json_path = dest.join("boot.v1.json");

        if let Ok(cont) = fs::read_to_string(&json_path) {
            if let Ok(parsed) = serde_json::from_str(&cont) {
                json = Some(parsed)
            }
        }
    }

    Ok(json.unwrap())
}

pub fn parse_generation(generation: &str) -> Result<(usize, Option<String>)> {
    if PROFILE_RE.is_match(generation) {
        let caps = PROFILE_RE.captures(generation).unwrap();
        let i = caps["generation"].parse::<usize>()?;

        Ok((i, Some(caps["profile"].to_string())))
    } else if SYSTEM_RE.is_match(generation) {
        let caps = SYSTEM_RE.captures(generation).unwrap();
        let i = caps["generation"].parse::<usize>()?;

        Ok((i, None))
    } else {
        Err("generation wasn't a system or profile generation".into())
    }
}
