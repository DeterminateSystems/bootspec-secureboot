// this just creates generation boot configs
// accepts a list of system profiles / generations

// TODO: better error handling
//  -> just replace unwraps with expects for now

// to create the bootloader profile:
// (do this in the installer package?)
// 1. cd (mktemp -d)
// 2. run this to get boot/entries/...
// 3. nix-store --add ./somepath
// 4a. make sure bootloader profile doesn't exist (or is ours)?
// 4b. then nix-env -p /nix/var/nix/profiles/bootloader --set ...

// TODO: think about user flows, how the tool should behave
// use cases:
//   * generating bootloader entries to install (if supported / necessitated by the bootloader)

// boot.loader.manual.enable = true; <- stubs out the `installBootloader` script to say "OK, update your bootloader now!\n  {path to bootspec.json}"

use std::path::PathBuf;

use generator::grub;
use generator::systemd_boot::{self, SigningInfo};
use generator::Result;

#[derive(Default, Debug)]
struct Args {
    // TODO: --out-dir?
    /// TODO
    signing_info: Option<SigningInfo>,
    /// TODO
    generations: Vec<String>,
}

fn main() -> Result<()> {
    let args = self::parse_args()?;

    if args.generations.is_empty() {
        return Err("expected list of generations".into());
    }

    for generation in &args.generations {
        if generation.is_empty() {
            continue;
        }

        let (i, profile) = generator::parse_generation(generation)?;
        let generation_path = PathBuf::from(&generation);
        let json = generator::get_json(&generation_path)?;

        let signing_info = &args.signing_info;

        systemd_boot::generate(&json, i, &profile, &generation_path, signing_info)?;

        grub::entry(&json, i, &profile).unwrap();
    }

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut pico = pico_args::Arguments::from_env();

    if pico.contains(["-h", "--help"]) {
        // TODO: help
        // print!("{}", HELP);
        std::process::exit(0);
    }

    let signing_key: Option<PathBuf> = pico.opt_value_from_str("--signing-key")?;
    let signing_cert: Option<PathBuf> = pico.opt_value_from_str("--signing-cert")?;
    let objcopy: Option<PathBuf> = pico.opt_value_from_os_str("--objcopy", self::parse_path)?;
    let sbsign: Option<PathBuf> = pico.opt_value_from_os_str("--sbsign", self::parse_path)?;
    let signing_info = match (signing_key, signing_cert, objcopy, sbsign) {
        (None, None, None, None) => None,
        (Some(signing_key), Some(signing_cert), Some(objcopy), Some(sbsign)) => Some(SigningInfo {
            signing_key,
            signing_cert,
            objcopy,
            sbsign,
        }),
        _ => {
            return Err("--signing-key, --signing-cert, --objcopy, and --sbsign are all required when signing for SecureBoot".into());
        }
    };

    let args = Args {
        signing_info,
        generations: pico
            .finish()
            .into_iter()
            .map(|s| s.into_string().expect("invalid utf8 in generation"))
            .collect(),
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<PathBuf> {
    Ok(s.into())
}
