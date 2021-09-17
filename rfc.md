---
feature: bootloader_specification
start-date: (fill me in with today's date, YYYY-MM-DD)
author: Cole Helbling (@cole-h)
co-authors: Graham Christensen (@grahamc)
shepherd-team: (names, to be nominated and accepted by RFC steering committee)
shepherd-leader: (name to be appointed by RFC steering committee)
related-issues: (will contain links to implementation PRs)
---

# Summary
[summary]: #summary

<!-- One paragraph explanation of the feature. -->

The goal of this feature is to distill and generalize the information that the various NixOS bootloader scripts consume into a single document attached to the generation.

# Motivation
[motivation]: #motivation

<!--
Why are we doing this? What use cases does it support? What is the expected
outcome?
-->

In Nixpkgs / NixOS, there exist various bootloader tools (such as those used for managing systemd-boot and grub), each utilizing varying amounts of information about the generation. As it is now, bootloader tools may spelunk the filesystem in order to infer necessary information, such as the kernel version or where the initrd is located. By creating a baseline document that contains this information in a machine-parsable format, these tools can instead rely on the generation's description of itself.

This document would also make it possible and easy for users to create their own bootloader, customized to their unique needs. Instead of needing to copy the current implementation for, say, systemd-boot, and then make further changes from there (inheriting all the complexity), they could start from scratch (and even in another language!). For example, if a user wanted to implement Secure Boot support in their bootloader: they may want to send the files necessary for boot (e.g. the kernel, initrd, and init itself) to an external server for signing. With the current infrastructure, this would be difficult -- the user would need to patch the current `systemd-boot-builder.py` script.

The goal of this RFC can be summed up into X points:

1. To attach a description of necessary boot information to all (future) generations
1. To limit filesystem magic in bootloader tools by utilizing that description
1. To require a further RFC in order to change the contents
1. TODO: (meh:) unified input information via that same document? (e.g. every bootloader gets the same information -- additional info they want is up to them)
1. TODO: more?


# Detailed design
[design]: #detailed-design

<!--
This is the core, normative part of the RFC. Explain the design in enough
detail for somebody familiar with the ecosystem to understand, and implement.
This should get into specifics and corner-cases. Yet, this section should also
be terse, avoiding redundancy even at the cost of clarity.
-->

The proposed bootloader specification document takes the form of a JSON document with a filename `boot.v#.json`, where `#` is the current major version number, (referred to henceforth as `boot.json`) and the contents:

- `init` (build-time)
  - The path to the generation's stage 2 init
  - Build-time because the `init` is written directly to the generation's toplevel (which is only reachable via `$out`)
- `initrd` (eval-time)
  - The store path of the generation's initrd
  - `"${config.system.build.initialRamdisk}/${config.system.boot.loader.initrdFile}"`
- `initrdSecrets` (eval-time)
  - The generation's `append-initrd-secrets` binary
  - `"${config.system.build.initialRamdiskSecretAppender}/bin/append-initrd-secrets"`
- `kernel` (eval-time)
  - The store path of the generation's kernel
  - `"${config.boot.kernelPackages.kernel}/${config.system.boot.loader.kernelFile}"`
- `kernelParams` (eval-time)
  - A JSON list of parameters to pass to the kernel
  - `config.boot.kernelParams`
- `kernelVersion` (eval-time)
  - The version of the generation's kernel
  - `config.boot.kernelPackages.kernel.modDirVersion`
- `schemaVersion`
  - The version of the bootloader schema described by the boot.json
- `specialisations` (eval-time)
  - A JSON mapping of specialisation names to the location of their boot.json
- `systemVersion` (eval-time)
  - The generation's NixOS version
  - `config.system.nixos.label`
- `toplevel` (build-time)
  - The store path of the generation's toplevel
  - Build-time because the toplevel path is only reachable via `$out`

JSON was chosen as the format because Nix already supports serializing and deserializing from this format extremely well (via `builtins.toJSON` and `builtins.fromJSON`), and many languages support -- or have libraries that support -- fiddling with JSON.

While it is desirable to have a good foundation of information to utilize, it is undesirable to bring the whole kitchen sink. Each of these keys was chosen by determining what information the current bootloader tools use and from that information deciding what would be most useful to be provided rather than having to be discovered.

Essentially, we want to limit filesystem magic to the bare minimum; it may be unavoidable for any additional information not present in the specification, but that which is generally necessary for a properly-functioning bootloader should be easily accessible.

This document would have both its filename and contents versioned in order to support potential future additions to (or removals from) the format.

TODO: versioning bounds?:
Adding a new key would only require a "minor" version bump (the `schemaVersion`)
Removing or renaming a key would require a "major" version bump (in the filename) -- reset `schemaVersion` to 1

TODO: examples of when a key should be added or removed? concrete guidance on it?


# Examples and Interactions
[examples-and-interactions]: #examples-and-interactions

<!--
This section illustrates the detailed design. This section should clarify all
confusion the reader has from the previous sections. It is especially important
to counterbalance the desired terseness of the detailed design; if you feel
your detailed design is rudely short, consider making this section longer
instead.
-->

A concrete example of the desire to limit "filesystem magic" is the `kernelVersion` key: both systemd-boot and grub bootloader tools use (or may use) the kernel version in the description of a generation. However, to retrieve this information, they must get the directory name of the kernel's modules path, which is done by code similar to the following shell snippet: `basename $(dirname $(realpath $toplevel/kernel))/lib/modules/*`.

Rather than maintaining the status quo of bootloader tools being essentially required to extract necessary information from the filesystem, we should instead hand this static information (most of which can be gathered at eval-time, anyways!) directly to the tool. For example, the kernel version is easily reachable at eval-time via the `config.boot.kernelPackages.kernel.modDirVersion` attribute.

## Example `boot.json`

One possible implementation generating the `boot.json` may be found here: https://github.com/DeterminateSystems/nixpkgs/tree/boot-spec-rfc.

```json
{
  "init": "/nix/store/067rp620j6x0l9rqz5cqa4m3dnd5k79k-nixos-system-scadrial-21.11.20210810.dirty/init",
  "initrd": "/nix/store/2p7dgp7zj3kgddcgrc94swrbfj2gdmah-initrd-linux-5.12.19/initrd",
  "initrdSecrets": "/nix/store/r2f307ky2n6ymn4hfs6av7vfy7y9vyid-append-initrd-secrets/bin/append-initrd-secrets",
  "kernel": "/nix/store/v3xankkp4lzd6cl7n4xs63d0pxdm90m0-linux-5.12.19/bzImage",
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
  "kernelVersion": "5.12.19-zen2",
  "schemaVersion": 1,
  "specialisation": {},
  "systemVersion": "21.11.20210810.dirty",
  "toplevel": "/nix/store/067rp620j6x0l9rqz5cqa4m3dnd5k79k-nixos-system-scadrial-21.11.20210810.dirty"
}
```

# Drawbacks
[drawbacks]: #drawbacks

<!-- Why should we *not* do this? -->

- This would make the `jq` package a dependency of the system builder script
- Implementing parsing of the bootloader specification in the current tools may require bringing in additional dependencies to deal with JSON


# Alternatives
[alternatives]: #alternatives

<!--
What other designs have been considered? What is the impact of not doing this?
-->

- Alternatives may include using a different, but easily machine-parsable language
  - JSON and XML are the only languages that Nix supports generating (e.g. using `builtins.toJSON` and `builtins.toXML`), but JSON was chosen because it has better tooling in a larger variety of language ecosystems


# Unresolved questions
[unresolved]: #unresolved-questions

<!-- What parts of the design are still TBD or unknowns? -->

- Should the specification be in the system's toplevel output, or should it be in a subdirectory, such as `nixos-support/`, `nix-support/`, `meta/`, ...?
- Are there any other keys that should be supported?


# Future work
[future]: #future-work

<!--
What future work, if any, would be implied or impacted by this feature
without being directly part of the work?
-->

Future work could include:
- Porting the existing bootloaders to parse the JSON instead of filesystem spelunking (if the document exists)
- A `boot.loader.custom` NixOS attribute that would allow people to write their own bootloader that consumes the specification
- Rewriting the existing bootloader tooling into a singular tool