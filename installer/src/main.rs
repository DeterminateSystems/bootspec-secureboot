// this will install the bootloader and should have bootloader-specific stuff

// NOTE: profile names might have invalid characters? https://github.com/NixOS/nixpkgs/pull/114637
// TODO: maybe make the installer use the generator directly? e.g. don't write to files, write to a HashMap<String, String>, which maps the file path to its contents

use std::error::Error;
use std::path::PathBuf;

mod grub;
mod systemd_boot;
mod util;

// TODO: separate by bootloader using a subcommand?
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
    /// TODO
    configuration_limit: Option<usize>,
    /// TODO
    editor: bool,

    // EFI-specific arguments
    /// The path to the EFI System Partition
    esp: Option<PathBuf>,
    /// Whether or not to touch EFI vars in the NVRAM
    can_touch_efi_vars: bool,
    /// TODO: bootctl path
    bootctl: Option<PathBuf>,
}

pub(crate) type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

fn main() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");

    let args = self::parse_args().unwrap();

    // TODO: choose which bootloader to install to somehow
    // (for now, hardcoded to systemd_boot for dogfood purposes)
    // TODO: better error handling (eyre? something with backtraces, preferably...)
    systemd_boot::install(args).unwrap();

    Ok(())
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
        toplevel: pico.value_from_fn("--toplevel", self::parse_path)?,
        dry_run: pico.contains("--dry-run"),
        generated_entries: pico.value_from_fn("--generated-entries", self::parse_path)?,
        timeout: pico.opt_value_from_str("--timeout")?,
        console_mode: pico.value_from_str("--console-mode")?,
        configuration_limit: pico.opt_value_from_str("--configuration-limit")?,
        editor: pico.opt_value_from_str("--editor")?.unwrap_or(true),

        // EFI-specific
        esp: pico.opt_value_from_fn("--esp", self::parse_path)?,
        can_touch_efi_vars: pico.contains("--touch-efi-vars"),
        bootctl: pico.opt_value_from_fn("--bootctl", self::parse_path)?,
    };

    dbg!(&args);

    Ok(args)
}

fn parse_path(s: &str) -> Result<PathBuf> {
    Ok(s.into())
}
