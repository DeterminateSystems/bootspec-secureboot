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

The goal of this feature is to distill and generalize the information that the various NixOS bootloader scripts consume into a single JSON document.

# Motivation
[motivation]: #motivation

<!--
Why are we doing this? What use cases does it support? What is the expected
outcome?
-->

general why:
- not ideal that grub script is perl, systemd-boot script is python, and extlinux script is bash and all use varying granularities of info about the system
  - by unifying the information they consume, easier to unify the scripts themselves; rewrite into a language that makes it easier to test
  - maybe this belongs in the future work section
-

what use cases:
- make it possible for people to make their own bootloader generator from a single source of truth about the generation
  - an example being somebody implementing a bootloader that supports secure boot
  - users have varying needs; say, they want to send the stuff to be signed to an external server, that doesn't necessarily need to be bound to the nixpkgs repo -- just take the information that all other tools have access to and do your own magic (and maybe publish on github somewhere for others to look at / experiment with / use)
-

what is the expected outcome:
- a JSON document that provides useful information for creating a bootloader script or for consumption in already-created bootloader scripts
- unified featureset / input information between bootloader scripts
  - e.g. grub supports stuff like a background image, custom fonts, etc from an external file, while systemd-boot doesn't
-


# Detailed design
[design]: #detailed-design

<!--
This is the core, normative part of the RFC. Explain the design in enough
detail for somebody familiar with the ecosystem to understand, and implement.
This should get into specifics and corner-cases. Yet, this section should also
be terse, avoiding redundancy even at the cost of clarity.
-->

>> format, contents, and meaning of data

```json
{
  "init": "/nix/store/067rp620j6x0l9rqz5cqa4m3dnd5k79k-nixos-system-scadrial-21.11.20210810.dirty-cosmere/init",
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
  "systemVersion": "21.11.20210810.dirty-cosmere",
  "toplevel": "/nix/store/067rp620j6x0l9rqz5cqa4m3dnd5k79k-nixos-system-scadrial-21.11.20210810.dirty-cosmere"
}
```

- most info should be retrievable at eval-time; very little at build time (exceptions: $toplevel, more?)
-


# Examples and Interactions
[examples-and-interactions]: #examples-and-interactions

<!--
This section illustrates the detailed design. This section should clarify all
confusion the reader has from the previous sections. It is especially important
to counterbalance the desired terseness of the detailed design; if you feel
your detailed design is rudely short, consider making this section longer
instead.
-->

TODO: publish / cleanup some work to detsys/nixpkgs

# Drawbacks
[drawbacks]: #drawbacks

<!-- Why should we *not* do this? -->

why not:
-


# Alternatives
[alternatives]: #alternatives

<!--
What other designs have been considered? What is the impact of not doing this?
-->

other designs:
-

impact of not doing this:
- bootloader scripts continue to diverge  until it becomes difficult to augment them in any meaningful way
-


# Unresolved questions
[unresolved]: #unresolved-questions

<!-- What parts of the design are still TBD or unknowns? -->

TBD:
-


# Future work
[future]: #future-work

<!--
What future work, if any, would be implied or impacted by this feature
without being directly part of the work?
-->

implied / impacted:
- port existing bootloaders to parse the json for the stuff it provides
- "manual" bootloader
  - just like the other bootloaders, is ran with the toplevel dir as the only argument
  - TODO: more accurate name; maybe "custom"?
