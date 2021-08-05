// I'm imagining the installer will create e.g. systemd-boot's loader.conf
// and grub's grub.cfg (amongst other required files) and add it to the
// generated bootloader/ folder, then add it to the store, then update the
// bootloader profile to point to that store path
//
// bootloader profile should only consist of the generated entries?
//
// installer will take generated entries from /nix/var/nix/profiles/bootloader,
// atomically switch all entries (using .tmp and `mv` for systemd), then add
// the bootloader-specific config to the boot partition directly? hardware
// and related config will always be taken from the current system...
//
// a snapshot of the bootloader-specific files that should go into /boot

// collect a list of entries that we generate and remove old ones from ESP/loader/entries:
//     gens = get_generations()
//     for profile in get_profiles():
//         gens += get_generations(profile)
//     remove_old_entries(gens)

/*
1a. only (bare) arg is the path to the default / just-built toplevel
1b. maybe accept flags for stuff like timeout, etc, that goes into the config
2. read machine_id and append it to all entries? (thus removing machine_id handling from the generator... but that's not the best)
3a. NIXOS_INSTALL_GRUB and NIXOS_INSTALL_BOOTLOADER
3b. if N_I_B and loader/loader.conf exists in ESP (destination), remove it
3c. if canTouchEfiVars, bootctl install, else bootctl install --no-variables
4. else, update to latest version of sd-boot (compare systemd and installed sd-boot versions)
5. get a list of entries to generate, also check profiles, and remove old entries
6. write loader conf if one of the generations' store dir (realpath) matches the toplevel we were passed
7. special-case memtest? bleh
8. syncfs to make sure a crash/outage doesn't make the system unbootable
*/

use std::env;
use std::path::{Path, PathBuf};

use crate::{Result,Args};

pub(crate) fn install(args: Args) -> Result<()> {
    let _ = (args.toplevel, args.esp, args.can_touch_efi_vars, args.dry_run, args.generated_entries);
    let _: PathBuf = PathBuf::new();

    // purposefully don't support NIXOS_INSTALL_GRUB because it's legacy, and this tool isn't :)
    match env::var("NIXOS_INSTALL_BOOTLOADER") {
        Ok(var) if var == "1" => {
            // installing bootloader
            if Path::new("@efisys@/loader/loader.conf").exists() {
                // remove it
            }
            if args.can_touch_efi_vars {
                // bootctl install --path=@efisys@
            } else {
                // bootctl install --no-variables --path=@efisys@
            }
        }
        _ => {
            // updating bootloader (if necessary)
            // get systemd version and installed bootloader version
            // check /boot/EFI/systemd/systemd-bootx64.efi for "#### LoaderInfo: systemd-boot (\\d+) ####" string
            // maybe use https://docs.rs/grep-searcher/0.1.8/grep_searcher/index.html to search the binary file or reimplement https://github.com/systemd/systemd/blob/32a2ee2bb4fa265577c883403748c909cd6784dd/src/boot/bootctl.c#L136
        }
    }

    Ok(())
}
