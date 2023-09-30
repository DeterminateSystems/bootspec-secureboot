use std::path::PathBuf;

use generator::bootable::{self, Bootable, EfiProgram};
use generator::{systemd_boot, Generation, Result};
use structopt::StructOpt;

#[derive(Default, Debug, StructOpt)]
struct Args {
    // TODO: --out-dir?
    /// The systemd-boot EFI stub used to create a unified EFI file
    #[structopt(long, requires_all = &["ukify", "unified-efi"])]
    systemd_efi_stub: Option<PathBuf>,
    /// The `ukify` binary
    #[structopt(long, requires_all = &["systemd-efi-stub", "unified-efi"])]
    ukify: Option<PathBuf>,
    /// Whether or not to combine the initrd and kernel into a unified EFI file
    #[structopt(long, requires_all = &["systemd-efi-stub", "ukify"])]
    unified_efi: bool,
    /// The `systemd-machine-id-setup` binary
    // TODO: maybe just pass in machine_id as an arg; if empty, omit from configuration?
    #[structopt(long)]
    systemd_machine_id_setup: PathBuf,
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
                .map(|(index, profile)| {
                    let bootspec = generator::get_json(PathBuf::from(gen));

                    bootspec
                        .map(|bootspec| Generation {
                            index,
                            profile,
                            bootspec,
                        })
                        .ok()
                })
                .flatten()
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

    systemd_boot::generate(
        bootables,
        args.ukify,
        args.systemd_efi_stub,
        args.systemd_machine_id_setup,
    )?;

    // TODO: grub
    // grub::generate(bootables, args.objcopy)?;

    Ok(())
}
