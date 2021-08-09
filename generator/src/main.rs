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

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix;
use std::path::{Path, PathBuf};

use generator::{grub, systemd_boot};

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    // TODO: --out-dir?
    // if len(args) < 2, quit
    // this will eventually accept a list of profiles / generations with which to generate bootloader configs
    let generations = env::args().skip(1);
    // basically [/nix/var/nix/profiles/system-69-link, /nix/var/nix/profiles/system-70-link, ...]

    for generation in generations {
        if generation.is_empty() {
            continue;
        }

        let (i, profile) = generator::parse_generation(&generation);
        let generation_path = PathBuf::from(&generation);
        let json = generator::get_json(generation_path);

        for (path, contents) in systemd_boot::entry(&json, i, &profile).unwrap() {
            fs::create_dir_all(format!("{}/efi/nixos", systemd_boot::ROOT)).unwrap();
            fs::create_dir_all(format!("{}/loader/entries", systemd_boot::ROOT)).unwrap();
            let mut f = fs::File::create(path).unwrap();
            write!(f, "{}", contents.conf).unwrap();

            if !Path::new(&contents.kernel.1).exists() {
                unix::fs::symlink(contents.kernel.0, contents.kernel.1).unwrap();
            }

            if !Path::new(&contents.initrd.1).exists() {
                unix::fs::symlink(contents.initrd.0, contents.initrd.1).unwrap();
            }
        }

        grub::entry(&json, i, &profile).unwrap();
    }
}
