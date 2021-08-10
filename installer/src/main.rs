// this will install the bootloader and should have bootloader-specific stuff

// NOTE: profile names might have invalid characters? https://github.com/NixOS/nixpkgs/pull/114637
// TODO: maybe make the installer use the generator directly? e.g. don't write to files, write to a HashMap<String, String>, which maps the file path to its contents

use std::error::Error;
use std::path::PathBuf;

mod grub;
mod systemd_boot;
mod util;

// TODO: separate by EFI and BIOS? or by bootloader using a subcommand?
#[derive(Default, Debug)]
struct Args {
    /// The path to the default configuration's toplevel.
    toplevel: PathBuf,
    /// Whether to actually touch stuff or not
    dry_run: bool,
    /// The directory that the generator created
    generated_entries: PathBuf,
    /// TODO
    timeout: Option<usize>,
    /// TODO
    console_mode: String,

    // EFI-specific arguments
    /// The path to the EFI System Partition
    esp: Option<PathBuf>,
    /// Whether or not to touch EFI vars in the NVRAM
    can_touch_efi_vars: bool,
    /// TODO: bootctl path
    bootctl: Option<PathBuf>,
}

pub(crate) type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

// TODO: check for root permissions -- required
fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    // installer
    //   --toplevel=...
    //   --esp=...
    //   --touch-efi-vars=...
    //   --dry-run=...
    //   --generated-entries=... <- path to generator's output dir
    let args = parse_args().unwrap();

    // TODO: choose which bootloader to install to somehow
    // (for now, hardcoded to systemd_boot for dogfood purposes)
    systemd_boot::install(args).expect("failed to install");
}

fn parse_args() -> Result<Args> {
    let mut pico = pico_args::Arguments::from_env();
    // TODO: pico subcommand per supported bootloader

    if pico.contains(["-h", "--help"]) {
        // TODO: help
        // print!("{}", HELP);
        std::process::exit(0);
    }

    let args = Args {
        toplevel: pico.value_from_fn("--toplevel", parse_path)?,
        dry_run: pico.value_from_str("--dry-run").unwrap_or_default(),
        generated_entries: pico.value_from_fn("--generated-entries", parse_path)?,
        timeout: pico.opt_value_from_str("--timeout")?,
        console_mode: pico
            .value_from_str("--console-mode")
            .unwrap_or_else(|_| String::from("keep")),

        // EFI-specific
        esp: pico.opt_value_from_fn("--esp", parse_path)?,
        can_touch_efi_vars: pico
            .opt_value_from_str("--touch-efi-vars")?
            .unwrap_or_default(),
        bootctl: pico.opt_value_from_fn("--bootctl", parse_path)?,
    };

    dbg!(&args);

    Ok(args)
}

fn parse_path(s: &str) -> Result<PathBuf> {
    Ok(s.into())
}
