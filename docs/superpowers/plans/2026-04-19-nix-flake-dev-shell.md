# Nix Flake Dev Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `flake.nix` so that `nix develop` provides every dependency needed to cross-compile Plato for Kobo and run the emulator on macOS and Linux.

**Architecture:** Single `devShells.default` driven by two flake inputs (`nixpkgs-unstable` and `rust-overlay`). Linaro 4.9.4 is packaged in `nix/linaro.nix` using `pkgs.fetchurl` to download directly from Google Drive (separate tarballs for Darwin and Linux). On Linux, `autoPatchelfHook` patches the ELF binaries for the Nix environment. MuPDF 1.27.0 is provided by overriding the nixpkgs derivation. All other packages come straight from nixpkgs.

**Tech Stack:** Nix Flakes 2.4+, nixpkgs-unstable, oxalica/rust-overlay, Nix stdenv (Darwin + Linux)

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `flake.nix` | Create | Declare inputs, expose `devShells.default` for `aarch64-darwin`, `x86_64-darwin`, `x86_64-linux` |
| `nix/linaro.nix` | Create | Derivation that downloads and unpacks the correct Linaro tarball per platform |
| `.gitignore` | Modify (done ✓) | Ignore `nix/linaro-darwin.tar.bz2` (harmless; no local tarball needed) |
| `flake.lock` | Auto-generated | Pinned input revisions |

---

## Task 1: Stage the Linaro tarball and gitignore it

**Files:**
- Modify: `.gitignore`
- Create (not committed): `nix/linaro-darwin.tar.bz2`

- [ ] **Step 1: Create the nix/ directory**

```bash
mkdir -p nix
```

- [ ] **Step 2: Copy the Linaro tarball into place**

```bash
cp ~/Downloads/gcc-linaro-4.9.4-2017.01-20170615_darwin.tar.bz2 nix/linaro-darwin.tar.bz2
```

- [ ] **Step 3: Verify the SHA256 matches the expected value**

```bash
shasum -a 256 nix/linaro-darwin.tar.bz2
```

Expected output (the hash must match exactly):
```
ad23f8252fcab0af1dc4fc2f758ec29dbe73c5b29b1589d5b8a7bf5483887ff4  nix/linaro-darwin.tar.bz2
```

If it doesn't match, do not proceed — the tarball is corrupt or a different version.

- [ ] **Step 4: Add the tarball to .gitignore**

Append to `.gitignore`:
```
/nix/linaro-darwin.tar.bz2
```

- [ ] **Step 5: Commit the .gitignore change**

```bash
git add .gitignore
git commit -m "chore: ignore Linaro tarball in nix/"
```

---

## Task 2: Write nix/linaro.nix and verify the ARM compiler appears in PATH

**Files:**
- Create: `nix/linaro.nix`

- [ ] **Step 1: Write the derivation**

Create `nix/linaro.nix`:

```nix
{ pkgs, system }:
let
  inherit (pkgs) lib;

  sources = {
    "x86_64-darwin" = {
      url  = "https://drive.usercontent.google.com/download?id=1ggMLM3VBwCYQuFTpJEC0OmyMkiDtYMju&export=download&confirm=t";
      hash = "sha256-rSP4JS/KsK8dxPwvdY7Cnb5zxbKbFYnVuKe/VIOIf/Q=";
    };
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
```

- [ ] **Step 2: Write a minimal flake.nix that only includes the Linaro package (scaffold)**

Create `flake.nix`:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems       = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems f;
    in {
      devShells = forAllSystems (system:
        let
          pkgs   = import nixpkgs { inherit system; };
          linaro = import ./nix/linaro.nix { inherit pkgs system; };
        in {
          default = pkgs.mkShell {
            packages = [ linaro ];
          };
        }
      );
    };
}
```

- [ ] **Step 3: Enter the shell and verify arm-linux-gnueabihf-gcc is on PATH**

```bash
nix develop --command arm-linux-gnueabihf-gcc --version
```

Expected output (version line):
```
arm-linux-gnueabihf-gcc (Linaro GCC 4.9-2017.01) 4.9.4
```

- [ ] **Step 4: Commit**

```bash
git add nix/linaro.nix flake.nix
git commit -m "feat(nix): add Linaro 4.9.4 derivation and scaffold flake"
```

---

## Task 3: Add rust-overlay and the Rust toolchain

**Files:**
- Modify: `flake.nix`

- [ ] **Step 1: Verify cargo is NOT yet available (expected failure)**

```bash
nix develop --command cargo --version
```

Expected: error — cargo not found.

- [ ] **Step 2: Add rust-overlay input and Rust toolchain to flake.nix**

Replace `flake.nix` entirely:

```nix
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
        in {
          default = pkgs.mkShell {
            packages = [ linaro rust ];
          };
        }
      );
    };
}
```

- [ ] **Step 3: Update flake.lock to pin rust-overlay**

```bash
nix flake update rust-overlay
```

- [ ] **Step 4: Verify cargo and rustc are available**

```bash
nix develop --command cargo --version
nix develop --command rustc --version
```

Expected output (exact versions will vary, check for stable channel):
```
cargo 1.x.x (... ...)
rustc 1.x.x (... ...)
```

- [ ] **Step 5: Verify ARM target is present**

```bash
nix develop --command rustc --print target-list | grep arm-unknown-linux-gnueabihf
```

Expected: `arm-unknown-linux-gnueabihf`

- [ ] **Step 6: Commit**

```bash
git add flake.nix flake.lock
git commit -m "feat(nix): add Rust stable toolchain with arm-unknown-linux-gnueabihf target"
```

---

## Task 4: Add SDL2, FreeType, HarfBuzz, DjVuLibre

**Files:**
- Modify: `flake.nix`

- [ ] **Step 1: Verify SDL2 is NOT yet available (expected failure)**

```bash
nix develop --command pkg-config --modversion sdl2
```

Expected: error — sdl2 not found.

- [ ] **Step 2: Add the four packages to flake.nix**

Replace the `packages` list in `flake.nix`:

```nix
packages = [
  linaro
  rust
  pkgs.SDL2
  pkgs.freetype
  pkgs.harfbuzz
  pkgs.djvulibre
];
```

- [ ] **Step 3: Verify all four packages are available**

```bash
nix develop --command pkg-config --modversion sdl2
nix develop --command pkg-config --modversion freetype2
nix develop --command pkg-config --modversion harfbuzz
nix develop --command pkg-config --modversion ddjvuapi
```

Each command must exit 0 and print a version string.

- [ ] **Step 4: Commit**

```bash
git add flake.nix
git commit -m "feat(nix): add SDL2, FreeType, HarfBuzz, DjVuLibre for emulator"
```

---

## Task 5: Add MuPDF 1.27.0

**Files:**
- Modify: `flake.nix`

- [ ] **Step 1: Verify mupdf is NOT yet available (expected failure)**

```bash
nix develop --command pkg-config --modversion mupdf
```

Expected: error — mupdf not found.

- [ ] **Step 2: Replace flake.nix entirely with the MuPDF override added**

```nix
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
          mupdf' = pkgs.mupdf.overrideAttrs (_old: {
            version = "1.27.0";
            src = pkgs.fetchurl {
              url  = "https://casper.mupdf.com/downloads/archive/mupdf-1.27.0-source.tar.gz";
              hash = "sha256-xuffhESEBMoT0Yio1b6jx6wr+sYmpTctGJnkbUFCJK4=";
            };
            patches = [];
          });
        in {
          default = pkgs.mkShell {
            packages = [
              linaro
              rust
              pkgs.SDL2
              pkgs.freetype
              pkgs.harfbuzz
              pkgs.djvulibre
              mupdf'
            ];
          };
        }
      );
    };
}
```

- [ ] **Step 3: Build the override (this compiles MuPDF from source — may take a few minutes)**

```bash
nix develop --command pkg-config --modversion mupdf
```

Expected: version string `1.27.0`.

If the override fails to build due to API changes between the nixpkgs MuPDF version and 1.27.0, also override `buildInputs` to clear version-dependent extras:

```nix
mupdf' = pkgs.mupdf.overrideAttrs (_old: {
  version = "1.27.0";
  src = pkgs.fetchurl {
    url  = "https://casper.mupdf.com/downloads/archive/mupdf-1.27.0-source.tar.gz";
    hash = "sha256-xuffhESEBMoT0Yio1b6jx6wr+sYmpTctGJnkbUFCJK4=";
  };
  patches    = [];
  buildFlags = [ "HAVE_X11=no" "HAVE_GLUT=no" "shared=yes" ];
});
```

- [ ] **Step 4: Commit**

```bash
git add flake.nix
git commit -m "feat(nix): add MuPDF 1.27.0 override"
```

---

## Task 6: Add thirdparty build tools

**Files:**
- Modify: `flake.nix`

These tools are required by the scripts in `thirdparty/` when cross-compiling the C/C++ dependencies (MuPDF, HarfBuzz, DjVuLibre, etc.) for Kobo.

- [ ] **Step 1: Verify meson is NOT yet available (expected failure)**

```bash
nix develop --command meson --version
```

Expected: error — meson not found.

- [ ] **Step 2: Add build tools to the packages list in flake.nix**

```nix
packages = [
  linaro
  rust
  pkgs.SDL2
  pkgs.freetype
  pkgs.harfbuzz
  pkgs.djvulibre
  mupdf'
  # thirdparty build tools
  pkgs.gnumake
  pkgs.meson
  pkgs.ninja
  pkgs.cmake
  pkgs.autoconf
  pkgs.automake
  pkgs.libtool
  pkgs.pkg-config
];
```

- [ ] **Step 3: Verify all build tools are present**

```bash
nix develop --command bash -c "
  make    --version | head -1 &&
  meson   --version &&
  ninja   --version &&
  cmake   --version | head -1 &&
  autoconf --version | head -1 &&
  automake --version | head -1 &&
  libtool  --version | head -1 &&
  pkg-config --version
"
```

Each tool must print a version without errors.

- [ ] **Step 4: Commit**

```bash
git add flake.nix
git commit -m "feat(nix): add thirdparty build tools (make, meson, ninja, cmake, autotools)"
```

---

## Task 7: Integration smoke test

These commands verify that the full shell satisfies all requirements from `doc/BUILD.md`.

- [ ] **Step 1: Check flake structure is valid**

```bash
nix flake check
```

Expected: exits 0 with no errors.

- [ ] **Step 2: Verify cross-compiler full triple**

```bash
nix develop --command bash -c "
  arm-linux-gnueabihf-gcc   --version | head -1 &&
  arm-linux-gnueabihf-g++   --version | head -1 &&
  arm-linux-gnueabihf-ar    --version | head -1 &&
  arm-linux-gnueabihf-strip --version | head -1
"
```

Expected: each prints a Linaro GCC 4.9.4 version line.

- [ ] **Step 3: Verify Rust + ARM target**

```bash
nix develop --command bash -c "
  rustc --version &&
  cargo --version &&
  rustc --target arm-unknown-linux-gnueabihf --print cfg 2>&1 | grep target_arch
"
```

Expected: `target_arch=\"arm\"`

- [ ] **Step 4: Verify emulator native libs**

```bash
nix develop --command bash -c "
  pkg-config --modversion sdl2 &&
  pkg-config --modversion freetype2 &&
  pkg-config --modversion harfbuzz &&
  pkg-config --modversion mupdf
"
```

Expected: four version strings, mupdf must be `1.27.0`.

- [ ] **Step 5: Verify build tools**

```bash
nix develop --command bash -c "
  which make meson ninja cmake autoconf automake libtool pkg-config
"
```

Expected: eight paths, all inside the Nix store.

- [ ] **Step 6: Final commit**

```bash
git add flake.nix flake.lock nix/linaro.nix .gitignore
git commit -m "feat(nix): complete dev shell with all BUILD.md dependencies"
```

---

## Appendix: Known hashes and URLs

| Artifact | Google Drive ID | Hash (SRI) |
|---|---|---|
| Linaro Darwin (`gcc-linaro-4.9.4-2017.01-20170615_darwin.tar.bz2`) | `1ggMLM3VBwCYQuFTpJEC0OmyMkiDtYMju` | `sha256-rSP4JS/KsK8dxPwvdY7Cnb5zxbKbFYnVuKe/VIOIf/Q=` |
| Linaro Linux (`gcc-linaro-4.9.4-2017.01-x86_64_arm-linux-gnueabihf.tar.*`) | `1xSf7PzfmI2DD7RHsPhwSy-Ltm8gabblK` | `sha256-B6wRny+a/dla8tzL3Lu1U2lcAQeBtSQ4lT+W/RhBkSI=` |
| `mupdf-1.27.0-source.tar.gz` | — | `sha256-xuffhESEBMoT0Yio1b6jx6wr+sYmpTctGJnkbUFCJK4=` |

Download URL pattern: `https://drive.usercontent.google.com/download?id=<ID>&export=download&confirm=t`
