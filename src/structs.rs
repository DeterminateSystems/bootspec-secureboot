// nixos-boot-manager
//   --generation-info=[json path]
//   --bootloader-conf=[conf path]
//   --bootloader=[systemd-boot,grub,extlinux]
//   --bios/--efi

use std::collections::HashMap;
use std::path::PathBuf;

// facts of the system, independent of a generation
// info about the generation
// presentation vs installation
// json for generation, some other configuration for the bootloader
// "bootloader" nix profile for bootloader related config stuff
// bootloader has different lifecycle than nixos

// version filename and contents (e.g. profile-boot-entry.v1.json)
// synthesize a v0 from existing generations (prior to this tooling)
// generate entries concurrently for multiple bootloaders (e.g. GRUB2 and extlinux.conf)
//   (legacy boot GRUB 2 vs UEFI GRUB 2 may conflict)

// NOTE: timeout is duplicated between bootloaders because they may be semantically different

struct EfiConf {
    /// efiSysMountPoint -- ESP
    esp_mountpoint: String,
    /// Whether or not to write to EFI vars in NVRAM
    can_touch_efi: bool,
}

struct GrubUser {
    /// GRUB user username
    username: String,
    /// GRUB user hashedPassword
    hashedPassword: Option<String>,
    /// GRUB user hashedPasswordFile
    hashedPasswordFile: Option<String>,
    /// GRUB user password
    password: Option<String>,
    /// GRUB user passwordFile
    passwordFile: Option<String>,
}

struct SharedGrubConf {
    /// Background color used to fill areas the image isn't covering
    backgroundColor: String,
    /// ID of bootloader to store in NVRAM (if allowed to write to EFI)
    bootloaderId: String,
    /// Path of the bootloader
    bootPath: String,
    /// Whether or not to copy kernels from the store to the bootloader
    copyKernels: bool,
    /// Index of the default boot entry
    default_entry: usize, // maybe isize? idk if grub supports negative indices as the default entry
    /// Devices to install GRUB to
    devices: Vec<String>,
    /// Extra GRUB configuration
    extraConfig: String,
    /// Extra GRUB boot entries
    extraEntries: Vec<String>,
    /// Extra GRUB boot entries that should appear before NixOS
    extraEntriesBeforeNixOS: Vec<String>,
    /// Modify the invocation of `grub-install`
    extraGrubInstallArgs: Vec<String>,
    /// Extra GRUB configuration added to each entry
    extraPerEntryConfig: String,
    /// Extra shell commands to run when preparing the bootloader
    extraPrepareConfig: Vec<String>,
    /// Font used in boot menu
    font: String,
    /// Force install even if problems are detected
    forceInstall: bool,
    /// How GRUB should identify devices when generating config
    fsIdentifier: String,
    /// GRUB "full" name
    fullName: String,
    /// GRUB "full" version
    fullVersion: String,
    /// GRUB version
    grubVersion: String,
    // PATH for installer Perl script
    // path: String, // TODO: unnecessary -- just subst in store paths
    /// shell to run various commands
    shell: String,
    /// GRUB background
    splashImage: String,
    /// Mode of GRUB background
    splashMode: String,
    /// Path of the Nix store when not copying kernels to the bootloader
    storePath: Option<String>, // iff copyKernels == false
    /// GRUB theme
    theme: String,
    /// Append entries detected by os-prober
    useOSProber: bool,
    /// GRUB users
    users: Vec<GrubUser>,
    /// time til autoboot -- maybe usize? idk if any bootloader support negative timeouts (e.g. to mean "instant boot")
    timeout: isize,
}

struct BiosGrubConf {
    /// gfxmode when BIOS boot
    gfxmodeBios: String,
    /// gfxpayload when BIOS boot
    gfxpayloadBios: String,
    /// GRUB package
    grub: String,
    /// Target GRUB is compiled for
    grubTarget: String,
    /// Shared GRUB config
    grub_conf: SharedGrubConf,
}
struct EfiGrubConf {
    /// If GRUB should install itself to a "hardcoded" location that firmwares must check
    efiInstallAsRemovable: bool,
    /// gfxmode when EFI boot
    gfxmodeEfi: String,
    /// gfxpayload when EFI boot
    gfxpayloadEfi: String,
    /// EFI version of GRUB 2
    grubEfi: String,
    /// Target EFI version of GRUB is compiled for
    grubTargetEfi: String,
    /// Shared GRUB config
    grub_conf: SharedGrubConf,
}

struct SystemdBootConf {
    /// Recent systemd versions require a machine ID
    machine_id: String,
    /// Whether or not to enable editing the kernel parameters
    editor: bool,
    /// Resolution of the console
    console_mode: String,
    // all these should just get substituted in with @path@ stuff
    // memtest86: Option<String>, // optional path to memtest86? maybe remove special case and just add ability to add custom entries
    // nix: String, // Nix path for listing generations?
    // systemd: String, // systemd path for generating machine_id, bootctl
    /// time til autoboot -- maybe usize? idk if any bootloader support negative timeouts (e.g. to mean "instant boot")
    timeout: isize,
}

struct ExtLinuxConf {
    /// time til autoboot -- maybe usize? idk if any bootloader support negative timeouts (e.g. to mean "instant boot")
    timeout: isize,
}

// JSON = generation config / info / data / fjdklsajkfl
// and then some out-of-band bootloader-specific config (maybe managed by a "bootloader" nix profile)

// JSON
struct SpecialisationName(String);
struct SystemConfigurationRoot(PathBuf);
struct BootJsonPath(PathBuf);

struct GenerationBucket {
    /// generation number -- killed
    // generation: usize,
    /// list of kernel parameters
    kernel_params: Vec<String>,
    /// NixOS version
    version: String,
    /// generation's build time / date -- killed
    // build_date: chrono::DateTime<chrono::Local>,
    /// config.system.build.toplevel path
    toplevel: SystemConfigurationRoot,
    /// profile name, if not "system" -- maybe not Option? -- killed
    // profile: Option<String>,
    /// Generation description -- default "NixOS {nixos-version}, Linux Kernel {kernel-version}, Built on {date}" -- killed
    // description: String,
    /// Path to "append-initrd-secrets" script -- $toplevel/append-initrd-secrets
    initrd_secrets: String,
    /// Path to kernel (bzImage) -- $toplevel/kernel
    kernel: String,
    /// Kernel version
    kernel_version: String,
    /// Path to initrd -- $toplevel/initrd
    initrd: String,
    /// initrd checksum
    initrd_sha256: String,
    /// Mapping of specialisation names to their configuration's boot.json -- to add all specialisations as a boot entry
    specialisation: HashMap<SpecialisationName, BootJsonPath>,
}

struct BootloaderBucket {
    // bootloader-specific config
    /// BIOS-specific GRUB config (includes "shared" GRUB config)
    bios_grub: Option<BiosGrubConf>,
    /// common EFI-specific config
    efi_common: Option<EfiConf>,
    /// EFI-specific GRUB config (includes "shared" GRUB config)
    efi_grub: Option<EfiGrubConf>,
    /// systemd-boot config
    systemdboot: Option<SystemdBootConf>,
    // ExtLinux config
    extlinux: Option<ExtLinuxConf>,

    // global config
    /// configuration limit
    limit: usize,
}

enum Bootloader {
    SystemdBoot, // BootloaderBucket.systemdboot + BootloaderBucket.efi_common
    // LegacyGrub, // ? -- maybe the same as bios_grub but with pre-v2 grub?
    BiosGrub, // BootloaderBucket.bios_grub
    EfiGrub,  // BootloaderBucket.efi_grub + BootloaderBucket.efi_common
    ExtLinux, // BootloaderBucket.extlinux
}
