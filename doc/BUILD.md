## Plato

### Preliminary

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

### Build phase

```sh
git clone https://github.com/baskerville/plato.git
cd plato
./build.sh
```

### Distribution

```sh
./dist.sh
```

## Emulator and importer

Install the required dependencies: *MuPDF 1.16.1*, *DjVuLibre*, *HarfBuzz*, *OpenJPEG*, *jpeg*, *jbig2dec*, *zlib*.

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
