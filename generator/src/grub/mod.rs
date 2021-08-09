use crate::{BootJson, Result};

// FIXME: placeholder dir
const ROOT: &str = "grub-entries";

pub fn entry(json: &BootJson, generation: usize, profile: &Option<String>) -> Result<()> {
    entry_impl(json, generation, profile, None)?;

    Ok(())
}

fn entry_impl(
    json: &BootJson,
    generation: usize,
    profile: &Option<String>,
    specialisation: Option<&str>,
) -> Result<()> {
    let _ = (json, generation, profile, specialisation);
    // TODO: UUID can be retrieved from `lsblk -no UUID {device path}` or `findmnt --first-only --noheadings --output UUID /boot`
    // TODO: support the xen stuff

    // schema: default entry has `- Default` in name and `--unrestricted`
    // what install-grub.pl does: create default entry: `"NixOS - Default" --unrestricted`
    // then create entries for all specialisations: "NixOS - (specialisation - {date} - {version})"
    // then submenu for all generations: "NixOS - Generation {i} ({date} - {version})" -- notably, no specialisations for prior generations?
    let data = format!(
        r#"menuentry "NixOS{}
        "#,
        "asdf"
    );

    let _ = (data, ROOT);

    Ok(())
}

// Generate the entries, but have the installer create the overall grub.cfg
// write to grub.entries file, pass that to the installer?
/*
fn grub_entry(json: &BootJson) {
    let data = format!(
        r#"menuentry "NixOS - {profile}" {options} {{
{search}
@extraPerEntryConfig@
multiboot {{xen}} {{xenparams}} if xen
module {{kernel}} if xen
module {{initrd}} if xen
linux {linux} {params}
initrd {initrd}
}}
"#,
        profile = "Default",
        options = "--unrestricted",
        search = "--set=drive1 --fs-uuid ASJD-NLSA",
        linux = json.kernel,
        params = json.kernel_params.join(" "),
        initrd = json.initrd,
    );

    println!("{}", data);
}
*/
