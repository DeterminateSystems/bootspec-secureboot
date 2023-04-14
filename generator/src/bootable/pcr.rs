use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PcrPhase {
    pub phase_path: String,
    pub banks: Vec<String>,
    pub private_key_file: PathBuf,
    pub public_key_file: PathBuf,
}
