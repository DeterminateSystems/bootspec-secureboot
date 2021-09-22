use std::path::PathBuf;

use generator::bootable::{self, Bootable, EfiProgram};
use generator::{systemd_boot, Generation, Result};
use structopt::StructOpt;

#[derive(Default, Debug, StructOpt)]
struct Args {
    // TODO: --out-dir?
    /// The systemd-boot EFI stub used to create a unified EFI file
    #[structopt(long, requires_all = &["objcopy", "unified-efi"])]
    systemd_efi_stub: Option<PathBuf>,
    /// The `objcopy` binary
    #[structopt(long, requires_all = &["systemd-efi-stub", "unified-efi"])]
    objcopy: Option<PathBuf>,
    /// Whether or not to combine the initrd and kernel into a unified EFI file
    #[structopt(long, requires_all = &["systemd-efi-stub", "objcopy"])]
    unified_efi: bool,
    /// A list of generations in the form of `/nix/var/nix/profiles/system-*-link`
    #[structopt(required = true)]
    generations: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let generations = args
        .generations
        .into_iter()
        .filter_map(|gen| {
            generator::parse_generation(&gen)
                .ok()
                .map(|(index, profile)| Generation {
                    index,
                    profile,
                    bootspec: generator::get_json(PathBuf::from(gen)),
                })
        })
        .collect::<Vec<_>>();
    let toplevels = bootable::flatten(generations)?;
    let bootables: Vec<Bootable> = if args.unified_efi {
        toplevels
            .into_iter()
            .map(|toplevel| Bootable::Efi(EfiProgram::new(toplevel)))
            .collect()
    } else {
        toplevels.into_iter().map(Bootable::Linux).collect()
    };

    systemd_boot::generate(bootables, args.objcopy, args.systemd_efi_stub)?;

    // TODO: grub
    // grub::generate(bootables, args.objcopy)?;

    Ok(())
}
