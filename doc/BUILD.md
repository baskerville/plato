# Build

Start by cloning the repository:

```sh
git clone https://github.com/baskerville/plato.git
cd plato
```

There are two ways to build *Plato*:
- [Local *Rust* Setup](#local)
- [With *Docker*/*Podman*](#docker)

## Local

### Plato

#### Preliminary

Install the appropriate [compiler toolchain](https://github.com/kobolabs/Kobo-Reader/tree/master/toolchain) (the binaries of the `bin` directory need to be in your path).

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

Install the required dependencies: *MuPDF 1.20.0*, *DjVuLibre*, *FreeType*, *HarfBuzz*.

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

## Docker

### Plato

1. Build the image for armhf: `docker build . -t plato:armhf`
2. The following compiles, mounts a local volume, and outputs the `plato` binary to your local folder `target/arm-unknown-linux-gnueabihf`:

```sh
docker run --rm -t -v $(pwd)/target:/plato/target plato:armhf
```

You can copy the binary to your Kobo device (make sure you install an existing release first) and it will run.

### Developer Tools

1. Build the image for dev environments: `docker build . -f Dockerfile.dev -t plato:dev`
2. The following runs tests, compiles, mounts a local volume, and outputs all binaries to your local folder `target/debug`:

```sh
docker run --rm -t -v $(pwd):/plato plato:dev
```

If the emulator or importer fail to run, please follow the steps in [Local–Developer Tools](#developer-tools) to ensure you have the relevant libraries.
