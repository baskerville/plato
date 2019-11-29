## Preliminary

Install the appropriate [compiler toolchain](https://github.com/kobolabs/Kobo-Reader/tree/master/toolchain) (the binaries of the `bin` directory need to be in your path).

Install the required dependencies: `curl`, `git`, `libtool`, `automake`, `pkg-config`, `cmake`, `ragel`, `patchelf`.

Install *rustup*:
```sh
curl https://sh.rustup.rs -sSf | sh
```

Install the appropriate target:
```sh
rustup target add arm-unknown-linux-gnueabihf
```

## Build Phase

```sh
git clone https://github.com/baskerville/plato.git
cd plato
```

### Fast Method

```sh
./build.sh fast
```

### Slow Method

If you want to build the thirdparty dependencies (instead of using the prebuilt ones), you shall use this method:

```sh
./build.sh slow
```

## Distribution

```sh
./dist.sh
```
