## Preliminary

Install [Ubuntu 12.04.5](http://releases.ubuntu.com/12.04/).

Install the required packages:
```sh
sudo apt-get upgrade
sudo apt-get install curl git libtool {auto,c}make ragel
sudo apt-get install {zlib1g,libjpeg8,libjbig2dec0}-dev
sudo apt-get install g{cc,++}-arm-linux-gnueabihf
```

Install *rustup*:
```sh
curl https://sh.rustup.rs -sSf | sh
```

Install the appropriate target:
```sh
rustup target add arm-unknown-linux-gnueabihf
```

Create *cargo*'s configuration file:
```sh
touch ~/.cargo/config
```

And append the following contents to it:
```toml
[target.arm-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
rustflags = ["-C", "target-feature=+v7,+vfp3,+a9,+neon"]
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
