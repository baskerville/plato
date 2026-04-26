# Build

Start by cloning the repository:

```sh
git clone https://github.com/baskerville/plato.git
cd plato
```

## Nix (recommended)

A `flake.nix` is provided that supplies every dependency automatically on macOS and Linux.

Enable flakes (once, user-level):
```sh
mkdir -p ~/.config/nix
echo 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```

Enter the development shell:
```sh
nix develop
```

This gives you the Linaro cross-compiler, Rust stable with the ARM target, and all libraries needed for the emulator. From inside the shell, the usual commands apply:

```sh
./build.sh        # cross-compile for Kobo
./run-emulator.sh # run the emulator
./dist.sh         # build the distribution archive
```

---

## Manual Setup

## Plato

#### Preliminary

Install the appropriate [compiler toolchain](https://drive.google.com/drive/folders/1YT6x2X070-cg_E8iWvNUUrWg5-t_YcV0) (the binaries of the `bin` directory need to be in your path).

Install the required dependencies: `wget`, `curl`, `git`, `pkg-config`, `unzip`, `jq`, `patchelf`.

Install *rustup*:
```sh
curl https://sh.rustup.rs -sSf | sh
```

Install the appropriate target:
```sh
rustup target add arm-unknown-linux-gnueabihf
```

### Build Phase

```sh
./build.sh
```

### Distribution

```sh
./dist.sh
```

## Developer Tools

Install the required dependencies: *MuPDF 1.27.0*, *DjVuLibre*, *FreeType*, *HarfBuzz*.

### Emulator

Install one additional dependency: *SDL2*.

You can then run the emulator with:
```sh
./run-emulator.sh
```

### Importer

You can install the importer with:
```sh
./install-importer.sh
```
