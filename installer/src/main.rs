// this will install the bootloader and should have bootloader-specific stuff

// NOTE: profile names might have invalid characters? https://github.com/NixOS/nixpkgs/pull/114637

use std::error::Error;
use std::path::PathBuf;

mod grub;
mod systemd_boot;

// TODO: separate by EFI and BIOS? or by bootloader
#[derive(Default, Debug)]
struct Args {
    /// The path to the default configuration's toplevel.
    toplevel: PathBuf,
    /// The path to the EFI System Partition
    esp: PathBuf,
    /// Whether or not to touch 
    can_touch_efi_vars: bool,
    /// Whether to actually touch stuff or not
    dry_run: bool,
    /// The directory that the generator created
    generated_entries: PathBuf,
}

pub(crate) type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

fn main() {
    // installer
    //   --toplevel=...
    //   --esp=...
    //   --touch-efi-vars=...
    //   --dry-run=... 
    //   --generated-entries=... <- path to generator's output dir
    let args = parse_args().unwrap();

    // TODO: choose which bootloader to install to somehow
    // (for now, hardcoded to systemd_boot for dogfood purposes)
    systemd_boot::install(args).unwrap();
}

fn parse_args() -> Result<Args> {
    let mut pico = pico_args::Arguments::from_env();

    if pico.contains(["-h", "--help"]) {
        // TODO: help
        // print!("{}", HELP);
        std::process::exit(0);
    }

    let args = Args {
        toplevel: pico.value_from_fn("--toplevel", parse_path)?,
        esp: pico.value_from_fn("--esp", parse_path)?,
        can_touch_efi_vars: pico.value_from_str("--touch-efi-vars").unwrap_or(false),
        dry_run: pico.value_from_str("--dry-run").unwrap_or(false),
        generated_entries: pico.value_from_fn("--generated-entries", parse_path)?,
    };

    dbg!(&args);

    Ok(args)
}

fn parse_path(s: &str) -> Result<PathBuf> {
    Ok(s.into())
}