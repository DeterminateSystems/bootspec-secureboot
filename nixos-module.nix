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
    };
  };
  config = {
    boot.loader.manual = {
      enable = true;
      installHook = pkgs.writeShellScript "install-bootloader"
        (
          let
            generatorArgs = lib.escapeShellArgs ([
              "--systemd-machine-id-setup"
              "${pkgs.systemd}/bin/systemd-machine-id-setup"
            ]
            ++ (lib.optionals config.boot.loader.secureboot.enable [
              "--unified-efi"

              "--objcopy"
              "${pkgs.binutils-unwrapped}/bin/objcopy"

              "--systemd-efi-stub"
              "${pkgs.systemd}/lib/systemd/boot/efi/linuxx64.efi.stub"
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
                "${pkgs.systemd}/bin/bootctl"

                "--generated-entries"
                "./systemd-boot-entries"
              ]
              ++ (lib.optional config.boot.loader.efi.canTouchEfiVariables "--touch-efi-vars")
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

            ${pkgs.bootspec}/bin/generator /nix/var/nix/profiles/system-*-link \
              ${generatorArgs}

            ${pkgs.bootspec}/bin/installer \
              --toplevel="$1" \
              ${installerArgs}
          ''
        );
    };
  };
}
