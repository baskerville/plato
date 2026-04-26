# Nix Flake Dev Shell — Design

## Overview

Add a `flake.nix` at the project root that provides a single `devShells.default` containing all
dependencies listed in `doc/BUILD.md`: the Linaro ARM cross-compiler, a Rust toolchain with the
ARM target, native libraries for the emulator, and all thirdparty build tools. Entering the shell
with `nix develop` is sufficient to run `./build.sh` and `./run-emulator.sh`.

The thirdparty source builds (`thirdparty/build.sh`) are not changed — they continue to
cross-compile their own libs. The flake provides the tools those scripts need (compiler, make,
meson, etc.) and the native libs the emulator links against.

---

## Files

### `flake.nix` (new)

Two inputs:

- `nixpkgs` — pinned to `nixpkgs-unstable`
- `rust-overlay` — `github:oxalica/rust-overlay`

The `outputs` function calls `rust-overlay.overlays.default` on nixpkgs, then defines
`devShells.default` for `aarch64-darwin` and `x86_64-darwin` (the project runs on macOS).

The shell's `packages` list:

| Package | Source |
|---|---|
| Linaro `arm-linux-gnueabihf` toolchain | `nix/linaro.nix` |
| Rust stable + `arm-unknown-linux-gnueabihf` target | `rust-overlay` — `pkgs.rust-bin.stable.latest.default.override { targets = ["arm-unknown-linux-gnueabihf"]; }` |
| SDL2 | `pkgs.SDL2` |
| FreeType | `pkgs.freetype` |
| HarfBuzz | `pkgs.harfbuzz` |
| DjVuLibre | `pkgs.djvulibre` |
| MuPDF 1.27.0 | `pkgs.mupdf` overridden to 1.27.0 (see below) |
| Build tools | `pkgs.gnumake`, `pkgs.meson`, `pkgs.ninja`, `pkgs.cmake`, `pkgs.autoconf`, `pkgs.automake`, `pkgs.libtool`, `pkgs.pkg-config` |

No `shellHook` is required — all packages expose their `bin/` directories automatically.

### `nix/linaro.nix` (new)

A single derivation that:

1. Fetches `gcc-linaro-4.9.4-2017.01-20170615_darwin.tar.bz2` using `pkgs.fetchurl`.
   The direct download URL must be resolved from the Google Drive folder in `doc/BUILD.md`
   during implementation (the folder ID is `1YT6x2X070-cg_E8iWvNUUrWg5-t_YcV0`; the per-file
   direct link uses the individual file ID, not the folder ID).
   SHA256 of the tarball: `sha256:1x3zi21m9gx7p3aqj5cvnb2p7glxqa77abzwqhfszc6a5wjzh8xd`
   (computed from the copy in `~/Downloads/`).
2. Unpacks the tarball into `$out` (strip one leading path component).
3. Sets `dontBuild = true`, `dontFixup = true` — the Darwin binaries need no patching on macOS.

The derivation is a function `{ pkgs }: pkgs.stdenv.mkDerivation { ... }` and is called from
`flake.nix` via `import ./nix/linaro.nix { inherit pkgs; }`.

### MuPDF 1.27.0 override

nixpkgs-unstable ships an older MuPDF. We override it inline in `flake.nix`:

```nix
mupdf' = pkgs.mupdf.overrideAttrs (_: {
  version = "1.27.0";
  src = pkgs.fetchurl {
    url = "https://casper.mupdf.com/downloads/archive/mupdf-1.27.0-source.tar.gz";
    # hash filled in during implementation with nix-prefetch-url
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
});
```

The source URL is the same one already used in `thirdparty/download.sh`.

---

## Cargo config

`.cargo/config.toml` already sets `linker = "arm-linux-gnueabihf-gcc"` for the ARM target. No
changes needed.

---

## Usage

```sh
nix develop          # enter the shell
./build.sh           # cross-compile for Kobo
./run-emulator.sh    # run the emulator
```

---

## Out of scope

- NixOS / Linux support (project is developed on macOS)
- Packaging the build outputs as Nix derivations
- CI integration
