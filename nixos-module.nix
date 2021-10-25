{ pkgs, lib, config, ... }:
{
  boot.loader.manual = {
    enable = true;
    installHook = pkgs.writeShellScript "install-bootloader"
      (
        let
          generatorArgs = lib.escapeShellArgs [
            "--systemd-machine-id-setup"
            "${pkgs.systemd}/bin/systemd-machine-id-setup"
          ];

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
}
