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
use std::fs::{self, File};
use std::io::Write;
// use std::os::unix;
use std::path::Path;
use std::process::Command;

// use generator::systemd_boot;
use grep_matcher::{Captures, Matcher};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::{Bytes, UTF8};
use grep_searcher::Searcher;

use crate::{util, Args, Result};

pub(crate) fn install(args: Args) -> Result<()> {
    let esp = args.esp.unwrap();
    let loader = format!("{}/loader/loader.conf", esp.display());

    let _ = (args.can_touch_efi_vars, args.dry_run);

    // purposefully don't support NIXOS_INSTALL_GRUB because it's legacy, and this tool isn't :)
    match env::var("NIXOS_INSTALL_BOOTLOADER") {
        Ok(var) if var == "1" => {
            if Path::new(&loader).exists() {
                fs::remove_file(&loader)?;
            }

            Command::new("bootctl")
                .args(&[
                    "install",
                    &format!("--path={}", &esp.display()),
                    if !args.can_touch_efi_vars {
                        "--no-variables"
                    } else {
                        ""
                    },
                ])
                .status()?;
        }
        _ => {
            // TODO: just use regex and parse the output of `bootctl --path=@esp@ status` lol why did I even think this was a good idea
            // regex: "^\\W+File:.*/EFI/(BOOT|systemd)/.*\\.efi \\(systemd-boot (\\d+)\\)$"
            let bootloader_version = {
                let f = File::open("/boot/EFI/systemd/systemd-bootx64.efi")?;
                let matcher =
                    RegexMatcher::new("#### LoaderInfo: systemd-boot (?P<version>\\d+) ####")?;
                let mut version = 0;

                Searcher::new().search_file(
                    &matcher,
                    &f,
                    Bytes(|_, line| {
                        // At this point, we are guaranteed to have a match: this closure is not
                        // entered unless the `matcher` finds a match inside `f`. Thus, all of these
                        // unwraps are safe.
                        let found = matcher.find(line)?.unwrap();
                        let found_line = &line[found];
                        let mut captures = matcher.new_captures()?;
                        matcher.captures(found_line, &mut captures)?;
                        let idx = matcher.capture_index("version").unwrap();
                        let version_capture = captures.get(idx).unwrap();

                        version = std::str::from_utf8(&found_line[version_capture])
                            .expect("version was invalid utf8")
                            .parse::<usize>()
                            .expect("version was not a number");

                        Ok(true)
                    }),
                )?;

                version
            };

            let systemd_version = {
                let output = Command::new("bootctl")
                    .arg("--version")
                    .output()
                    .expect("failed to execute bootctl")
                    .stdout;
                let matcher = RegexMatcher::new("systemd (?P<version>\\d+) \\(\\d+\\)")?;
                let mut version = 0;

                Searcher::new().search_slice(
                    &matcher,
                    &output,
                    UTF8(|_, line| {
                        // At this point, we are guaranteed to have a match: this closure is not
                        // entered unless the `matcher` finds a match inside `output`. Thus, all of
                        // these unwraps are safe.
                        let found = matcher.find(line.as_bytes())?.unwrap();
                        let found_line = &line[found];
                        let mut captures = matcher.new_captures()?;
                        matcher.captures(found_line.as_bytes(), &mut captures)?;
                        let idx = matcher.capture_index("version").unwrap();
                        let version_capture = captures.get(idx).unwrap();

                        version = found_line[version_capture]
                            .to_string()
                            .parse::<usize>()
                            .expect("version was not a number");

                        Ok(true)
                    }),
                )?;

                version
            };

            // TODO: compare versions and update if systemd is newer than bootloader
            let _ = (bootloader_version, systemd_version);
        }
    }

    // TODO: do we want to create the files here, and depend on the generator, or run the generator in a separate step?
    // fs::create_dir_all(format!("{}/efi/nixos", systemd_boot::ROOT))?;
    // fs::create_dir_all(format!("{}/loader/entries", systemd_boot::ROOT))?;

    // for generation in get_all_generations() {
    //     let (i, profile) = generator::parse_generation(&generation);
    //     let generation_path = PathBuf::from(&generation);
    //     let json = generator::get_json(generation_path);

    //     for (path, contents) in systemd_boot::entry(&json, i, &profile)? {
    //         let mut f = fs::File::create(path)?;
    //         write!(f, "{}", contents.conf)?;

    //         if !Path::new(&contents.kernel.1).exists() {
    //             unix::fs::symlink(contents.kernel.0, contents.kernel.1)?;
    //         }

    //         if !Path::new(&contents.initrd.1).exists() {
    //             unix::fs::symlink(contents.initrd.0, contents.initrd.1)?;
    //         }
    //     }
    // }

    util::copy_recursively(&args.generated_entries, &esp)?;

    for (idx, generation) in all_generations(None) {
        if fs::canonicalize(&generation)? == fs::canonicalize(&args.toplevel)? {
            let tmp_loader = format!("{}/loader/loader.conf.tmp", esp.display());
            let mut f = File::create(&tmp_loader)?;

            if let Some(timeout) = args.timeout {
                write!(f, "{}", timeout)?;
            }
            // if let Some(profile) = args.profile {
            //     // TODO: support system profiles?
            // } else {
            write!(f, "default nixos-generation-{}.conf", idx)?;
            // }
            // if let Some(editor) = args.editor {
            //     // TODO
            // }
            write!(f, "console-mode {}", args.console_mode)?;

            fs::rename(tmp_loader, &loader)?;
        }
    }

    Ok(())
}

fn all_generations(profile: Option<String>) -> Vec<(usize, String)> {
    let profile_path = if let Some(profile) = profile {
        format!("/nix/var/nix/profiles/system-profiles/{}", profile)
    } else {
        String::from("/nix/var/nix/profiles/system")
    };

    let mut generations = Vec::new();
    let output = String::from_utf8(
        Command::new("nix-env")
            .args(&["-p", &profile_path, "--list-generations"])
            .output()
            .expect("failed to execute nix-env")
            .stdout,
    )
    .expect("found invalid UTF-8");

    for line in output.lines() {
        let generation = line
            .trim()
            .split(' ')
            .next()
            .expect("couldn't find generation number");

        generations.push((
            generation.parse().expect("generation number was invalid"),
            format!("{}-{}-link", profile_path, generation),
        ));
    }

    generations
}
