{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }: {
    overlays.default = final: prev:
      let
        pkgs = prev;
      in {
        exomat = pkgs.callPackage (
          { rustPlatform
          , lib
          , installShellFiles
          }:
          rustPlatform.buildRustPackage {
            pname = "exomat";
            version = (lib.trivial.importTOML ./Cargo.toml).package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            # tests are not built for nix sandbox
            doCheck = false;
          }) {};

        exomat_static = (self.overlays.default final prev.pkgsStatic).exomat;

        # add potential packages (fpm?) here
        exomat_static_renamed = pkgs.callPackage (
          { stdenv }:
          let
            exomat = final.exomat_static;
            host_arch = pkgs.hostPlatform.linuxArch;
            package_name = "${exomat.pname}_${exomat.version}_${host_arch}";
          in stdenv.mkDerivation {
            pname = "exomat_static_renamed";
            version = exomat.version;

            dontUnpack = true;
            dontConfigure = true;
            dontBuild = true;

            buildInputs = [ exomat ];

            installPhase = ''
              runHook preInstall

              mkdir -p $out/bin
              cp -a ${exomat}/bin/exomat $out/bin/${package_name}

              runHook postInstall
            '';
          }) {};

        exomat_all_archs =
          let
            pkgs_archs = [
              (if "x86_64" == pkgs.hostPlatform.linuxArch then pkgs else pkgs.pkgsCross.gnu64)
              (if "amd64" == pkgs.hostPlatform.linuxArch then pkgs else pkgs.pkgsCross.aarch64-multiplatform)
            ];
          in pkgs.symlinkJoin {
            name = "exomat_archs";
            paths = builtins.map (pkgs: (pkgs.extend self.overlays.default).exomat_static_renamed) pkgs_archs;
          };
      };
  } //
  ( flake-utils.lib.eachDefaultSystem (system: 
    let
      pkgs = import nixpkgs { inherit system; };
      selfpkgs = self.packages."${system}";
    in {
      packages = (self.overlays.default selfpkgs pkgs) //
        { default = selfpkgs.exomat; };
    }));
}
