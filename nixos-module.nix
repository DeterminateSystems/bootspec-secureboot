{ pkgs, lib, config, ... }:
let
  inherit (lib) types;
in
{
  options = {
    boot.loader.secureboot = {
      enable = lib.mkEnableOption "Secure Boot support";
      signingKeyPath = lib.mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      signingCertPath = lib.mkOption {
        type = types.nullOr types.str;
        default = null;
      };

      pcrPhases = {
        enable = lib.mkEnableOption "pcr phases";

        signatures = lib.mkOption {
          default = { };
          type = types.attrsOf (types.submodule ({ name, ... }: {
            config.phasePath = lib.mkDefault name;
            options = {
              phasePath = lib.mkOption {
                type = types.str;
              };
              banks = lib.mkOption {
                type = types.listOf types.str;
                default = [ ];
              };
              privateKeyFile = lib.mkOption {
                type = types.path // { apply = toString; };
              };
              publicKeyFile = lib.mkOption {
                type = types.path // { apply = toString; };
              };
            };
          }));
        };
      };
    };
  };
  config = {
    boot.kernelParams = lib.mkIf config.boot.loader.secureboot.pcrPhases.enable [ "systemd.gpt_auto=false" ];
    boot.initrd = lib.mkIf config.boot.loader.secureboot.pcrPhases.enable {
      availableKernelModules = [ "efivarfs" ];
      systemd = {
        package = config.systemd.package;
        additionalUpstreamUnits = [ "systemd-pcrphase-initrd.service" ];
        services.systemd-pcrphase-initrd = {
          wantedBy = [ "initrd.target" ];
          after = [ "systemd-modules-load.service" ];

          # TODO: How should this be pulled in?
          wants = [ "cryptsetup-pre.target" ];
        };

        # TODO: This is sketchy, but works as long as no initrd FSes
        # are ordered before local-fs.target (zfs currently needlessly
        # does this in nixos)
        targets.cryptsetup-pre.after = [ "systemd-tmpfiles-setup.service" ];

        storePaths = [ "${config.boot.initrd.systemd.package}/lib/systemd/systemd-pcrphase" ];
        contents."/etc/tmpfiles.d/90-tpm-pcr-signature.conf".text = ''
          C /run/systemd/tpm2-pcr-signature.json - - - - /.extra/tpm2-pcr-signature.json
        '';
      };
    };
    systemd = lib.mkIf config.boot.loader.secureboot.pcrPhases.enable {
      additionalUpstreamSystemUnits = [
        "systemd-pcrphase-sysinit.service"
        "systemd-pcrphase.service"
      ];
      services.systemd-pcrphase-sysinit.wantedBy = [ "basic.target" ];
      services.systemd-pcrphase.wantedBy = [ "multi-user.target" ];
    };
    boot.loader.external = {
      enable = true;
      installHook = pkgs.writeShellScript "install-bootloader"
        (
          let
            generatorArgs = lib.escapeShellArgs ([
              "--systemd-machine-id-setup"
              "${config.systemd.package}/bin/systemd-machine-id-setup"
            ]
            ++ (lib.optionals config.boot.loader.secureboot.enable [
              "--unified-efi"

              "--objcopy"
              "${pkgs.binutils-unwrapped}/bin/objcopy"

              "--systemd-efi-stub"
              "${config.systemd.package}/lib/systemd/boot/efi/linuxx64.efi.stub"
            ] ++ lib.optionals config.boot.loader.secureboot.pcrPhases.enable [
              "--systemd-measure"
              "${config.systemd.package}/lib/systemd/systemd-measure"
              "--pcr-phases"
              (pkgs.writeText "pcr-phases" (builtins.toJSON (lib.mapAttrsToList (n: v: v) config.boot.loader.secureboot.pcrPhases.signatures)))
            ]));

            installerArgs = lib.escapeShellArgs
              ([
                "--esp"
                config.boot.loader.efi.efiSysMountPoint

                "--console-mode"
                config.boot.loader.systemd-boot.consoleMode

                "--timeout"
                (toString config.boot.loader.timeout)

                "--bootctl"
                "${config.systemd.package}/bin/bootctl"

                "--generated-entries"
                "./systemd-boot-entries"
              ]
              ++ (lib.optional config.boot.loader.efi.canTouchEfiVariables "--can-touch-efi-vars")
              ++ (lib.optionals (config.boot.loader.systemd-boot.configurationLimit != null) [
                "--configuration-limit"
                "${toString config.boot.loader.systemd-boot.configurationLimit}"
              ])
              ++ (lib.optionals (config.boot.loader.secureboot.signingKeyPath != null) [
                "--signing-key"
                config.boot.loader.secureboot.signingKeyPath
              ])
              ++ (lib.optionals (config.boot.loader.secureboot.signingCertPath != null) [
                "--signing-cert"
                config.boot.loader.secureboot.signingCertPath
              ])
              ++ (lib.optionals config.boot.loader.secureboot.enable [
                "--unified-efi"

                "--sbsign"
                "${pkgs.sbsigntool}/bin/sbsign"

                "--sbverify"
                "${pkgs.sbsigntool}/bin/sbverify"
              ]));
          in
          ''
            set -eux

            scratch=$(mktemp -d -t tmp.XXXXXXXXXX)
            function finish {
              rm -rf "$scratch"
            }
            trap finish EXIT

            cd "$scratch" || exit 1

            ${pkgs.bootspec-secureboot}/bin/generator /nix/var/nix/profiles/system-*-link \
              ${generatorArgs}

            ${pkgs.bootspec-secureboot}/bin/installer \
              --toplevel="$1" \
              $([ ! -z ''${NIXOS_INSTALL_BOOTLOADER+x} ] && echo --install) \
              ${installerArgs}
          ''
        );
    };
  };
}
