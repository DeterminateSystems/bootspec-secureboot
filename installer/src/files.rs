// TODO: better module name?

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::Result;

#[derive(Debug, PartialEq, Clone)]
pub struct FileToReplace {
    pub generated_loc: PathBuf,
    pub esp_loc: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IdentifiedFiles {
    pub to_sign: Vec<PathBuf>,
    pub to_replace: Vec<FileToReplace>,
}

impl IdentifiedFiles {
    pub fn new(generated_entries: &Path, esp: &Path) -> Result<Self> {
        let mut to_add = Vec::new();
        let mut to_replace = Vec::new();

        let generated_files = glob::glob(&format!("{}/**/*", generated_entries.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| !e.is_dir())
            .collect::<Vec<_>>();
        let esp_files = glob::glob(&format!("{}/**/*", esp.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| !e.is_dir())
            .collect::<Vec<_>>();

        let strip_unnecessary_prefix = |path: &Path| -> String {
            path.display()
                .to_string()
                .replace(&generated_entries.display().to_string(), "")
                .replace(&esp.display().to_string(), "")
        };

        for file in generated_files.iter().filter(|e1| {
            !esp_files
                .iter()
                .any(|e2| strip_unnecessary_prefix(e1) == strip_unnecessary_prefix(e2))
        }) {
            to_add.push(file.to_owned());
        }

        for generated_loc in generated_files {
            let stripped = strip_unnecessary_prefix(&generated_loc);

            if let Some(esp_loc) = esp_files
                .iter()
                .find(|e| stripped == strip_unnecessary_prefix(e))
                .map(ToOwned::to_owned)
            {
                to_replace.push(FileToReplace {
                    generated_loc,
                    esp_loc,
                })
            }
        }

        let to_sign = to_add
            .into_iter()
            .chain(to_replace.iter().map(|e| e.generated_loc.to_owned()))
            .filter(|e| e.extension() == Some(OsStr::new("efi")))
            .collect();

        Ok(IdentifiedFiles {
            to_sign,
            to_replace,
        })
    }
}
