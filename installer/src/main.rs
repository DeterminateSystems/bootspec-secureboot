// this will install the bootloader and should have bootloader-specific stuff

// NOTE: profile names might have invalid characters? https://github.com/NixOS/nixpkgs/pull/114637
// TODO: maybe make the installer use the generator directly? e.g. don't write to files, write to a HashMap<String, String>, which maps the file path to its contents
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

use log::LevelFilter;

use crate::secure_boot::SigningInfo;

mod files;
mod grub;
mod secure_boot;
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
    /// TODO
    verbosity: usize,
    /// TODO
    install: bool,

    // EFI-specific arguments
    /// The path to the EFI System Partition(s)
    esp: Vec<PathBuf>,
    /// Whether or not to touch EFI vars in the NVRAM
    can_touch_efi_vars: bool,
    /// TODO: bootctl path
    bootctl: Option<PathBuf>,
    /// Whether to use unified EFI files
    unified_efi: bool,
    /// The signing info used for Secure Boot
    signing_info: Option<SigningInfo>,
}

pub(crate) type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

fn main() -> Result<()> {
    let args = self::parse_args()?;

    env_logger::Builder::new()
        .format(|buf, record| writeln!(buf, "{:<5} {}", record.level(), record.args()))
        .filter(
            Some(env!("CARGO_PKG_NAME")), // only log for this
            match args.verbosity {
                0 => LevelFilter::Warn,
                1 => LevelFilter::Info,
                2 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            },
        )
        .try_init()?;

    // TODO: choose which bootloader to install to somehow
    // (for now, hardcoded to systemd_boot for dogfood purposes)
    // TODO: better error handling (eyre? something with backtraces, preferably...)
    systemd_boot::install(args)?;

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

    let mut verbosity = 0;
    while pico.contains(["-v", "--verbose"]) {
        verbosity += 1;
    }

    let signing_key = pico.opt_value_from_fn("--signing-key", self::parse_path_from_str)?;
    let signing_cert = pico.opt_value_from_fn("--signing-cert", self::parse_path_from_str)?;
    let sbsign = pico.opt_value_from_fn("--sbsign", self::parse_path_from_str)?;
    let sbverify = pico.opt_value_from_fn("--sbverify", self::parse_path_from_str)?;

    let signing_info = match (signing_key, signing_cert, sbsign, sbverify) {
        (None, None, None, None) => None,
        (Some(signing_key), Some(signing_cert), Some(sbsign), Some(sbverify)) => {
            Some(SigningInfo {
                signing_key,
                signing_cert,
                sbsign,
                sbverify,
            })
        }
        _ => {
            return Err("--signing-key, --signing-cert, --sbsign, and --sbverify are all required when signing for SecureBoot".into());
        }
    };

    let args = Args {
        toplevel: pico.value_from_fn("--toplevel", self::parse_path_from_str)?,
        dry_run: pico.contains("--dry-run"),
        generated_entries: pico.value_from_fn("--generated-entries", self::parse_path_from_str)?,
        timeout: pico.opt_value_from_str("--timeout")?,
        console_mode: pico.value_from_str("--console-mode")?,
        configuration_limit: pico.opt_value_from_str("--configuration-limit")?,
        editor: pico.opt_value_from_str("--editor")?.unwrap_or(true),
        verbosity,
        install: pico.contains("--install"),

        // EFI-specific
        esp: pico.values_from_fn("--esp", self::parse_path_from_str)?,
        can_touch_efi_vars: pico.contains("--touch-efi-vars"),
        bootctl: pico.opt_value_from_fn("--bootctl", self::parse_path_from_str)?,
        unified_efi: pico.contains("--unified-efi") || signing_info.is_some(),
        signing_info,
    };

    dbg!(&args);

    Ok(args)
}

fn parse_path_from_str(s: &str) -> Result<PathBuf> {
    Ok(s.trim_end_matches('/').into())
}
