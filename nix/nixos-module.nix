{ config, lib, pkgs, skeletonPackage, ... }:
let
  cfg = config.services.skeleton-that-runs-across-your-screen-sometimes;
in
{
  options.services.skeleton-that-runs-across-your-screen-sometimes = {
    enable = lib.mkEnableOption "the skeleton screen animation";
    package = lib.mkOption {
      type = lib.types.package;
      default = skeletonPackage;
      description = "Package to run in the graphical user session.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.skeleton-that-runs-across-your-screen-sometimes = {
      description = "Skeleton screen animation";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      serviceConfig.ExecStart = "${cfg.package}/bin/skeleton-that-runs-across-your-screen-sometimes";
    };
  };
}
