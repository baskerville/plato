# Setup

The OS used on the *Kobo* devices is *Linaro 2011.07*.

In order to build for this OS / architecture you can, for example, install *Ubuntu LTS 12.04* (the *GLIBC* version must be old enough) in a VM and install the following package: `gcc-4.6-arm-linux-gnueabihf`.

Install the appropriate target:
```sh
rustup target add arm-unknown-linux-gnueabihf
```

Append this:
```toml
[target.arm-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
rustflags = ["-C", "target-feature=+v7,+vfp3,+a9,+neon"]
```
to `~/.cargo/config`.

The binary can then be generated with:
```sh
cargo build --release --target=arm-unknown-linux-gnueabihf
```

You can tell what features are supported by your device from the output of `cat /proc/cpuinfo`.
