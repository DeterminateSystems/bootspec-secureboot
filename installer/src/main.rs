// this will install the bootloader and should have bootloader-specific stuff

// NOTE: profile names might have invalid characters? https://github.com/NixOS/nixpkgs/pull/114637
// TODO: maybe make the installer use the generator directly? e.g. don't write to files, write to a HashMap<String, String>, which maps the file path to its contents
use std::path::PathBuf;
use std::{error::Error, io::Write};

use log::LevelFilter;

mod files;
mod grub;
mod secure_boot;
mod systemd_boot;
mod util;

// TODO: separate by bootloader using a subcommand?
#[derive(clap::Parser, Default, Debug)]
struct Args {
    /// The path to the default configuration's toplevel.
    #[clap(long)]
    toplevel: PathBuf,
    /// Whether to actually touch stuff or not
    #[clap(long)]
    dry_run: bool,
    /// The directory that the generator created
    #[clap(long)]
    generated_entries: PathBuf,
    /// TODO
    #[clap(long)]
    timeout: Option<usize>,
    #[clap(long)]
    /// TODO
    console_mode: String,
    #[clap(long)]
    /// TODO
    configuration_limit: Option<usize>,
    /// TODO
    #[clap(long)]
    editor: bool,
    /// TODO
    #[clap(short, long, parse(from_occurrences))]
    verbosity: usize,
    /// TODO
    #[clap(long)]
    install: bool,

    #[clap(long)]
    // EFI-specific arguments
    /// The path to the EFI System Partition(s)
    esp: Vec<PathBuf>,
    /// Whether or not to touch EFI vars in the NVRAM
    #[clap(long)]
    can_touch_efi_vars: bool,
    #[clap(long)]
    /// TODO: bootctl path
    bootctl: Option<PathBuf>,
    /// Whether to use unified EFI files
    #[clap(long)]
    unified_efi: bool,
    #[clap(long, requires_all = &["signing-cert", "sbsign", "sbverify"])]
    /// The signing key used for Secure Boot
    signing_key: Option<PathBuf>,
    #[clap(long, requires_all = &["signing-key", "sbsign", "sbverify"])]
    /// The signing cert used for Secure Boot
    signing_cert: Option<PathBuf>,
    #[clap(long, requires_all = &["signing-key", "signing-cert", "sbverify"])]
    /// The sbsign binary to sign the files for Secure Boot
    sbsign: Option<PathBuf>,
    #[clap(long, requires_all = &["signing-key", "signing-cert", "sbsign"])]
    /// The sbverify binary to sign the files for Secure Boot
    sbverify: Option<PathBuf>,
}

pub(crate) type Result<T, E = Box<dyn Error + Send + Sync + 'static>> = core::result::Result<T, E>;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

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
