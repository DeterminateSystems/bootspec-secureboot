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

use generator::bootable::{self, Bootable, EfiProgram, Generation};
use generator::{systemd_boot, Result};

#[derive(Default, Debug)]
struct Args {
    // TODO: --out-dir?
    // FIXME: path to systemd-boot efi stub
    /// The `objcopy` binary
    ///
    /// Required if `--unified-efi` is provided
    objcopy: Option<PathBuf>,
    /// Whether or not to combine the initrd and kernel into a unified EFI file
    unified_efi: bool,
    /// A list of generations in the form of `/nix/var/nix/profiles/system-*-link`
    generations: Vec<String>,
}

fn main() -> Result<()> {
    let args = self::parse_args()?;

    let generations = args
        .generations
        .into_iter()
        .filter(|gen| generator::parse_generation(gen).is_ok())
        .map(|gen| {
            let (index, profile) = generator::parse_generation(&gen).unwrap();

            Generation {
                index,
                profile,
                json: generator::get_json(PathBuf::from(gen)),
            }
        })
        .collect::<Vec<_>>();
    let toplevels = bootable::flatten(generations, None)?;
    let bootables: Vec<Bootable> = if args.unified_efi {
        toplevels
            .into_iter()
            .map(|toplevel| Bootable::Efi(EfiProgram::new(toplevel)))
            .collect()
    } else {
        toplevels.into_iter().map(Bootable::Linux).collect()
    };

    systemd_boot::generate(bootables, args.objcopy)?;

    // TODO: grub
    // grub::generate(bootables, args.objcopy)?;

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut pico = pico_args::Arguments::from_env();

    if pico.contains(["-h", "--help"]) {
        // TODO: help
        // print!("{}", HELP);
        std::process::exit(0);
    }

    let args = Args {
        objcopy: pico.opt_value_from_os_str("--objcopy", self::parse_path)?,
        unified_efi: pico.contains("--unified-efi"),
        generations: pico
            .finish()
            .into_iter()
            .map(|s| s.into_string().expect("invalid utf8 in generation"))
            .collect(),
    };

    match (&args.objcopy, &args.unified_efi) {
        (None, false) => {}
        (Some(_), true) => {}
        _ => {
            return Err(
                "--objcopy and --unified-efi are required when one or the other is specified"
                    .into(),
            );
        }
    }

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<PathBuf> {
    Ok(s.into())
}
