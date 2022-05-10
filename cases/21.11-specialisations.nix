let
  src = builtins.fetchTarball "channel:nixos-21.11";
in
(import "${src}/nixos" {
  configuration = {
    imports = [
      "${src}/nixos/modules/virtualisation/qemu-vm.nix"
      ({ pkgs, ... }: {
        specialisation.example.configuration = {
          environment.systemPackages = [ pkgs.hello ];
        };
      })
    ];
  };
}).config.system.build.toplevel
