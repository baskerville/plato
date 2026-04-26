{ pkgs, system }:
let
  inherit (pkgs) lib;

  sources = {
    "x86_64-darwin" = {
      url  = "https://drive.usercontent.google.com/download?id=1ggMLM3VBwCYQuFTpJEC0OmyMkiDtYMju&export=download&confirm=t";
      hash = "sha256-rSP4JS/KsK8dxPwvdY7Cnb5zxbKbFYnVuKe/VIOIf/Q=";
    };
    # Both Darwin architectures use the same tarball because the Linaro Darwin binary
    # is x86_64 and runs under Rosetta 2 on Apple Silicon.
    "aarch64-darwin" = {
      url  = "https://drive.usercontent.google.com/download?id=1ggMLM3VBwCYQuFTpJEC0OmyMkiDtYMju&export=download&confirm=t";
      hash = "sha256-rSP4JS/KsK8dxPwvdY7Cnb5zxbKbFYnVuKe/VIOIf/Q=";
    };
    "x86_64-linux" = {
      url  = "https://drive.usercontent.google.com/download?id=1xSf7PzfmI2DD7RHsPhwSy-Ltm8gabblK&export=download&confirm=t";
      hash = "sha256-B6wRny+a/dla8tzL3Lu1U2lcAQeBtSQ4lT+W/RhBkSI=";
    };
  };
  source = sources.${system};
in
pkgs.stdenv.mkDerivation {
  name = "gcc-linaro-4.9.4-2017.01";

  src = pkgs.fetchurl {
    inherit (source) url hash;
  };

  # On Linux, autoPatchelfHook patches all ELF binaries to use Nix store paths.
  # On Darwin, Mach-O binaries need no patching.
  nativeBuildInputs = lib.optionals pkgs.stdenv.isLinux [
    pkgs.autoPatchelfHook
  ];

  buildInputs = lib.optionals pkgs.stdenv.isLinux [
    pkgs.glibc
    pkgs.stdenv.cc.cc.lib   # libstdc++, libgcc_s
  ];

  unpackPhase = ''
    tar -xf "$src" --strip-components=1 -C .
  '';

  installPhase = ''
    mkdir -p "$out"
    cp -r . "$out"
  '';

  dontBuild    = true;
  dontStrip    = true;
  dontFixup    = pkgs.stdenv.isDarwin;
  dontPatchELF = pkgs.stdenv.isDarwin;
}
