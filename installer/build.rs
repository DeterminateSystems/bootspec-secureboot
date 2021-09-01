use std::error::Error;
use std::process::Command;

type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

fn main() -> Result<()> {
    // This is to allow nix builds to substitute in the patched sbattach to avoid nix-inside-nix.
    let sbattach_str = String::from("@patched_sbattach@");
    let sbattach_out = if sbattach_str.starts_with('@') && sbattach_str.ends_with('@') {
        self::build_patched_sbattach()?
    } else {
        sbattach_str
    };

    println!(
        "cargo:rustc-env=PATCHED_SBATTACH_BINARY={}/bin/sbattach",
        sbattach_out
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=patched-sbattach.nix");
    println!("cargo:rerun-if-env-changed=PATH");

    Ok(())
}

fn build_patched_sbattach() -> Result<String> {
    let output = Command::new("nix-build")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/patched-sbattach.nix"))
        .arg("--no-out-link")
        .output()?;
    let stdout = std::str::from_utf8(&output.stdout)?.trim();

    Ok(stdout.to_owned())
}
