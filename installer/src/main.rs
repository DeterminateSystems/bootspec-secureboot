// this will install the bootloader and should have bootloader-specific stuff

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

fn main() {
    println!("Hello, world!");
}
