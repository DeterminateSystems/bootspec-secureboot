{ inputs
, lib
, nixosTest
, cryptsetup
, sbctl
, swtpm
, OVMFFull
, e2fsprogs
, libressl
, systemd
}:

nixosTest (
  let
    baseSecureBoot = {
      imports = [ inputs.self.nixosModules.bootspec-secureboot ];
      boot.loader.systemd-boot.enable = false;
      boot.loader.secureboot = {
        enable = true;
        signingKeyPath = "/etc/secureboot/keys/db/db.key";
        signingCertPath = "/etc/secureboot/keys/db/db.pem";

        pcrPhases = {
          enable = true;
          signatures."enter-initrd" = {
            privateKeyFile = "/etc/pcrphase-keys/tpm2-pcr-private.pem";
            publicKeyFile = "/etc/pcrphase-keys/tpm2-pcr-public.pem";
          };
        };
      };

      # Enable boot counting, because nixosTest's allow_reboot doesn't
      # crash on panic. Will fallback to a working config if we mess
      # with the /boot/loader/entries files a little bit.
      systemd.additionalUpstreamSystemUnits = [
        "systemd-bless-boot.service"
        "boot-complete.target"
        "systemd-boot-check-no-failures.service"
      ];
    };
  in
  {
    name = "pcr-test";

    nodes.machine = { config, ... }: {
      virtualisation = {
        emptyDiskImages = [ 512 ];
        useBootLoader = true;
        useEFIBoot = true;
        efi = {
          inherit (OVMFFull) firmware variables;
        };
        qemu.options = [ "-chardev socket,id=chrtpm,path=/tmp/mytpm1/swtpm-sock -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-tis,tpmdev=tpm0" ];
      };
      boot.loader.timeout = 0;
      boot.loader.efi.canTouchEfiVariables = true;
      boot.loader.systemd-boot.enable = lib.mkDefault true;
      boot.initrd.kernelModules = [ "tpm_tis" "tpm_crb" ];
      environment.systemPackages = [ cryptsetup sbctl e2fsprogs libressl ];
      boot.initrd.systemd.enable = true;

      specialisation.secureboot.configuration = baseSecureBoot;
      specialisation.unlock-cryptenroll.configuration = {
        imports = [ baseSecureBoot ];
        boot.initrd.luks.devices = lib.mkVMOverride {
          "foo".device = "/dev/vdc";
          "foo".crypttabExtraOpts = [ "tpm2-device=auto" "headless=true" ]; # tpm2-signature default should work
        };
        virtualisation.fileSystems."/foo" = {
          fsType = "ext4";
          autoFormat = true;
          device = "/dev/mapper/foo";
          neededForBoot = true;
        };
      };
    };

    testScript = ''
      import subprocess
      import os

      os.mkdir("/tmp/mytpm1")
      subprocess.Popen(
          [
              "${swtpm}/bin/swtpm",
              "socket", "--tpmstate", "dir=/tmp/mytpm1", "--ctrl",
              "type=unixio,path=/tmp/mytpm1/swtpm-sock",
              "--log", "level=20", "--tpm2"
          ],
          stdout=subprocess.DEVNULL,
          stderr=subprocess.DEVNULL
      )

      machine.start(allow_reboot=True)
      machine.wait_for_unit("multi-user.target")
      machine.fail("test -e /sys/firmware/efi/efivars/StubPcrKernelImage-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f")
      machine.succeed(
          "sbctl create-keys",
          "sbctl enroll-keys --yes-this-might-brick-my-machine",
          "mkdir /etc/pcrphase-keys",
          "openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out /etc/pcrphase-keys/tpm2-pcr-private.pem",
          "openssl rsa -pubout -in /etc/pcrphase-keys/tpm2-pcr-private.pem -out /etc/pcrphase-keys/tpm2-pcr-public.pem",
          "ln -s system-1-link /nix/var/nix/profiles/system",
          "ln -s $(readlink -f /run/current-system/specialisation/secureboot) /nix/var/nix/profiles/system-1-link",
          "ln -s $(readlink -f /run/current-system) /nix/var/nix/profiles/orig-system",
          "rm -vr /boot/*",
          "NIXOS_INSTALL_BOOTLOADER=1 /nix/var/nix/profiles/system/bin/switch-to-configuration boot",
          "sync",
      )
      print(machine.succeed("sbctl verify"))
      machine.reboot()

      machine.wait_for_unit("multi-user.target")
      machine.succeed(
          "test -e /sys/firmware/efi/efivars/LoaderEntrySelected-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f",
          "test -e /sys/firmware/efi/efivars/StubPcrKernelImage-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f",
          "test -e /run/systemd/tpm2-pcr-signature.json",

          "echo somepass | cryptsetup luksFormat --type=luks2 /dev/vdc",
          "dd if=/dev/urandom of=/etc/keyfile bs=32 count=1",
          "echo somepass | cryptsetup luksAddKey --new-keyfile=/etc/keyfile /dev/vdc",
          "systemd-cryptenroll --unlock-key-file=/etc/keyfile --tpm2-device=auto --tpm2-public-key=/etc/pcrphase-keys/tpm2-pcr-public.pem --tpm2-public-key-pcrs=11 --tpm2-pcrs=0+2+7 /dev/vdc",
          "rm /nix/var/nix/profiles/system",
          "ln -s $(readlink -f /nix/var/nix/profiles/orig-system/specialisation/unlock-cryptenroll) /nix/var/nix/profiles/system-2-link",
          "ln -s system-2-link /nix/var/nix/profiles/system",
          "/nix/var/nix/profiles/system/bin/switch-to-configuration boot",
          "sync",
      )

      machine.reboot()
      machine.wait_for_unit("multi-user.target")
      machine.succeed(
          # Test that the LUKS device was unlocked.
          "test -e /dev/mapper/foo",
          "umount /foo",
          "cryptsetup close foo",
      )
      # TPM should not allow unlocking this outside initrd
      machine.fail("${systemd}/lib/systemd/systemd-cryptsetup attach foo /dev/vdc - tpm2-device=auto,headless=true,tpm2-signature=/run/systemd/tpm2-pcr-signature.json")
      machine.succeed(
          # Reset keys to make sure it won't unlock.
          "sbctl reset",
          # Mess with the loader entries to enable boot counting.
          "mv /boot/loader/entries/nixos-generation-2.conf /boot/loader/entries/nixos-generation-2+3.conf",
          "mv /boot/loader/entries/nixos-generation-1.conf /boot/loader/entries/nixos-generation-1+3.conf",
      )

      # With keys reset, the LUKS disk should fail because of PCR 7, so
      # wait for systemd-boot boot counting to fallback to the previous
      # generation
      machine.reboot()
      machine.wait_for_unit("systemd-bless-boot.service")
      machine.succeed("[ $(readlink -f /run/current-system) = $(readlink -f /nix/var/nix/profiles/system-1-link) ]")
      machine.fail("test -e /dev/mapper/foo")
    '';
  }
)
