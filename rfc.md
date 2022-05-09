# RFC: BootSpec

Created: May 6, 2022 2:16 PM
Last Edited Time: May 9, 2022 10:04 AM
Stakeholders: Anonymous
Type: Technical Spec

# Summary

Create a stable, comprehensive, and machine-parsable definition of a NixOS Generation as an intermediate representation (IR) between the NixOS system definition and the bootloader management tools.

# Motivation

NixOS’s bootloader backends don’t support a uniform collection of features and design decisions, partially due to the complexity of implementing the features. Using a statically parsable bootspec definition reduces the work involved in implementing bootloader support.

If we survey the current set of bootloader and feature matrix, we see a bit of a smattering:

| Bootloader | Generation limits | Initrd Secrets | Multiple Profiles | Specialisation | Custom Labels |
| --- | --- | --- | --- | --- | --- |
| systemd-boot | YES | YES | YES | YES | YES |
| grub | YES | YES | YES | YES | YES |
| generic-extlinux-compatible | YES | NO | NO | NO | YES |
| generations-dir | NO | NO | NO | NO | NO |
| init-script | NO | NO | NO | YES | NO |
| raspberrypi | YES | NO | NO | NO | NO |

One reason the matrix is not filled out is the technical difficulty of implementing these various features. The current API for detecting most of this information is by globbing directories looking for specific files.

By forcing implementations to spelunk the filesystem, we create a complicated problem in multiple ways:

- Introducing or improving bootloader features is complicated and requires significant effort to implement it for the major bootloaders.
- The relatively complicated filesystem-based behavior involves reimplementing similar logic across Perl, Python, and Bash.
- Our current tools can only support limited workflows, and are difficult to extend.

## Supporting Externalized Bootloader Backends

Our NixOS-provided tooling is sufficient for most use cases, however there are more complicated cases that are not adequately covered by our tools and would not be appropriate to include in NixOS directly. Let’s use SecureBoot as an example.

A naive, single-user implementation of SecureBoot has signing keys on the local filesystem, and every `nixos-rebuild boot` call signs the relevant files. This is a valid method of implementation, however it isn’t sufficient for larger deployments.

An enterprise deployment of SecureBoot probably has a centralized signing service with careful auditing about what is signed. It is more likely that the signed bootloader files come prebuilt and presigned from upstream and an unsigned file means a violation of policy. This is also a valid implementation, but is too heavy for a single user.

There are infinitely many policy choices and implementations possible, and a good solution here is to allow deployers of NixOS to implement their own bootloader management tools. By creating a well defined specification of generations and the boot-relevant data we enable this external development.

## What about systemd's Bootloader Specification?

Systemd’s bootloader specification is a good format for a different problem. A single NixOS generation can contain multiple bootable systems and options, with additional features unique to NixOS built on top. Most Linux distributions don’t deal with many unique and ever-changing bootables. This proposal is specifically to deal with the collection of bootables and improve our ability to interface with.

# Goals

- Enable a more uniform bootloader feature support across our packaged bootloaders. Concretely, converting most of the NO’s in the feature matrix to YES’s.
- Enable users of NixOS to implement custom bootloader tools and policy without needing to dive through the system profiles, and without patching Nixpkgs / NixOS.
- Define a stable specification of a generation’s boot data which internal and external users can rely on. Changing to the specification should go through an RFC.

### Non-Goals

- Rewriting the existing bootloaders to actually fill out the feature matrix. The goal of this RFC is to make the feature development *easier*, not actually do it.
- Supporting SecureBoot. The authors of this RFC have done work in this regard, but this RFC is not about SecureBoot.

# Proposed Solution

- Each NixOS generation will have a bootspec (a JSON document) at `$out/boot.v1.json` containing all of the boot properties for that generation. NixOS’s bootloader backends will read these files as inputs to the bootloader installation phase.
- The bootloader backends will avoid reading data from the other files and directories when possible, preferring the information in the bootspec.
- A bootspec synthesizing tool will be used to synthesize a bootspec for generations which don’t already have one. This tool will be shared across all of the bootloader backends, helping produce more uniform behavior.
- Existing bootloader backends will be updated to read properties from the bootspec, removing most if not all of their filesystem-spelunking code.

### Bootspec Format v1

Using the following JSON:

```yaml
{
	# Version of the specification used in the document
  "schemaVersion": 1,

  # Path to the stage-2 init, executed by the initrd
  "init": "/nix/store/xxx-nixos-system-xxx/init",

	# Path to the initrd
  "initrd": "/nix/store/xxx-initrd-linux/initrd",

	# Optional path to a tool to dynamically add secrets to an initrd
  "initrdSecrets": "/nix/store/xxx-append-secrets/bin/append-initrd-secrets",

	# Path to the kernel image
  "kernel": "/nix/store/xxx-linux/bzImage",

	# Kernel commandline options
  "kernelParams": [
    "amd_iommu=on",
    "amd_iommu=pt",
    "iommu=pt",
    "kvm.ignore_msrs=1",
    "kvm.report_ignored_msrs=0",
    "udev.log_priority=3",
    "systemd.unified_cgroup_hierarchy=1",
    "loglevel=4"
  ],

	# Kernel version for display
  "kernelVersion": "5.12.19-zen2",

  # The version of the system, known  as `config.system.nixos.label`
  "systemVersion": "21.11.20210810.dirty",

  # Top level path of the closure, in case some spelunking is required
  "toplevel": "/nix/store/xxx-nixos-system-xxx",

	#
  "specialisation": {
    # <name> corresponds to <name> in specialisation.<name>.configuration.
    # Note: a specialisation's bootspec document should not contain any specialisations.
    "<name>": {
      # bootspec is optional, the path to a bootspec document
      # It is optional to account for closures which do not
      # have a bootspec and where one must be generated.
      "bootspec": "/path/to/a/bootspec.v1.json",

      # Path to the top-level path of the closure, in case further spelunking is required.
      "toplevel": "/nix/store/xxx-nixos-system-yyy"
    }
  }
}
```

### Risks

- Some of the bootloader backends are quite complicated, and in many cases have inadequate tests. We could accidentally break corner cases.
- The bootloader backends are inherently a weak point for NixOS, as it is our last option for rolling back. We cannot roll back a broken bootloader. This and the previous point are risks, but also help demonstrate the value of reducing the amount of code and complexity in the generator.

### Milestones

- Create and package the backwards compatibility synthesizer as a standalone tool. A version of it already exists, but it is not standalone.
- Generate the bootspec files as part of building the system closure.
- Update the bootloader backends to use bootspec as its primary source of installation data.
- Implement a NixOS module which supports external bootloader tooling.

# Open Questions

- Having `specialisation.<name>.bootspec` optional may be a mistake.

# Future Work

- Completing the migration from filesystem-spelunking into using the bootspec data.
- Implementing a NixOS module for supporting externalized bootloader backends.
- Implementing a base level of SecureBoot support.
