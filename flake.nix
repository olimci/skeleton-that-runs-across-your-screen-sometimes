{
  description = "A skeleton runs across your screen sometimes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forSystems = nixpkgs.lib.genAttrs systems;
      linuxInputs = pkgs: with pkgs; [
        alsa-lib
        libX11
        libXcursor
        libXi
        libXinerama
        libXrandr
        libxkbcommon
        wayland
        wayland-protocols
      ];
      packageArgs = pkgs: {
        pname = "skeleton-that-runs-across-your-screen-sometimes";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        strictDeps = true;
        nativeBuildInputs = with pkgs; [ cmake pkg-config ];
        buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (linuxInputs pkgs);
      };
      mkPackage = pkgs: pkgs.rustPlatform.buildRustPackage (packageArgs pkgs);
      mkWindowsPackage = pkgs: pkgs.rustPlatform.buildRustPackage ((packageArgs pkgs) // {
        doCheck = false;
      });
      mkCargoCheck = pkgs: command: extraNativeBuildInputs:
        pkgs.rustPlatform.buildRustPackage ((packageArgs pkgs) // {
          pname = "${(packageArgs pkgs).pname}-${command}";
          nativeBuildInputs = (packageArgs pkgs).nativeBuildInputs ++ extraNativeBuildInputs;
          doCheck = false;
          buildPhase = ''
            runHook preBuild
            cargo ${command} --locked --all-targets ${if command == "clippy" then "--all-features" else ""}
            runHook postBuild
          '';
          installPhase = "mkdir -p $out";
        });
    in
    {
      packages = forSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          windows = if builtins.elem system [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" ] then {
            windows-x86_64 = mkWindowsPackage (import nixpkgs {
            localSystem = system;
            crossSystem = {
              config = "x86_64-w64-mingw32";
              libc = "msvcrt";
            };
            });
          } else { };
        in
        {
          default = mkPackage pkgs;
        } // windows);

      checks = forSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          cargo-check = mkCargoCheck pkgs "check" [ ];
          clippy = mkCargoCheck pkgs "clippy" [ pkgs.clippy ];
        });

      nixosModules.default = { config, lib, pkgs, ... }:
        import ./nix/nixos-module.nix {
          inherit config lib pkgs;
          skeletonPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        };

      devShells = forSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [ cargo rustc clippy cmake pkg-config ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
              alsa-lib
              libX11
              libXcursor
              libXi
              libXinerama
              libXrandr
              libxkbcommon
              wayland
              wayland-protocols
            ]);
          };
        });
    };
}
