{
  inputs = {
    nixpkgs.url  = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url    = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems       = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems f;
    in {
      devShells = forAllSystems (system:
        let
          pkgs   = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          linaro = import ./nix/linaro.nix { inherit pkgs system; };
          rust   = pkgs.rust-bin.stable.latest.default.override {
            targets = [ "arm-unknown-linux-gnueabihf" ];
          };
          mupdf' = pkgs.mupdf.overrideAttrs (old: {
            version = "1.27.0";
            src = pkgs.fetchurl {
              url  = "https://casper.mupdf.com/downloads/archive/mupdf-1.27.0-source.tar.gz";
              hash = "sha256-riRCQW3kmRgtN6UmxvorrMejvtWoiNETygSERITf58Y=";
            };
            # postInstall =
            #   (builtins.replaceStrings ["Version: 1.27.2"] ["Version: 1.27.0"] old.postInstall)
            #   + ''
            #     # plato-core/build.rs emits -lmupdf-third on non-ARM targets, but this
            #     # MuPDF build bundles all third-party symbols into libmupdf.
            #     # An empty stub satisfies the linker without providing any symbols.
            #     ar rcs $out/lib/libmupdf-third.a
            #   '';
          });
        in {
          default = pkgs.mkShell {
            packages = [
              linaro
              rust
              # thirdparty build tools
              pkgs.git
              pkgs.gnumake
              pkgs.meson
              pkgs.ninja
              pkgs.cmake
              pkgs.autoconf
              pkgs.automake
              pkgs.libtool
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.SDL2
              pkgs.freetype
              pkgs.harfbuzz
              pkgs.djvulibre
              mupdf'
              mupdf'.dev
              # libs needed by plato-core/build.rs for the native emulator target
              pkgs.libjpeg
              pkgs.libpng
              pkgs.gumbo
              pkgs.openjpeg
              pkgs.jbig2dec
              pkgs.bzip2
              pkgs.zlib
            ];
            shellHook = ''
              export CC_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc
              export CXX_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++
              export AR_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-ar
            '';
          };
        }
      );
    };
}
