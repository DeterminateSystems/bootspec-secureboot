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

use std::fs;
use std::io::Write;
use std::os::unix;
use std::path::{Path, PathBuf};

use generator::{grub, systemd_boot, Result};

#[derive(Default, Debug)]
struct Args {
    // TODO: --out-dir?
    /// TODO
    generations: Vec<String>,
}

fn main() -> Result<()> {
    let args = self::parse_args()?;

    for generation in args.generations {
        if generation.is_empty() {
            continue;
        }

        let (i, profile) = generator::parse_generation(&generation);
        let generation_path = PathBuf::from(&generation);
        let json = generator::get_json(generation_path);

        for (path, contents) in systemd_boot::entry(&json, i, &profile)? {
            fs::create_dir_all(format!("{}/efi/nixos", systemd_boot::ROOT))?;
            fs::create_dir_all(format!("{}/loader/entries", systemd_boot::ROOT))?;
            let mut f = fs::File::create(path)?;
            write!(f, "{}", contents.conf)?;

            if !Path::new(&contents.kernel.1).exists() {
                unix::fs::symlink(contents.kernel.0, contents.kernel.1)?;
            }

            if !Path::new(&contents.initrd.1).exists() {
                unix::fs::symlink(contents.initrd.0, contents.initrd.1)?;
            }
        }

        grub::entry(&json, i, &profile)?;
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

    let args = Args {
        generations: pico
            .finish()
            .into_iter()
            .map(|s| s.into_string().expect("invalid utf8 in generation"))
            .collect(),
    };

    Ok(args)
}
