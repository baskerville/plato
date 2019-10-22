## Install

Extract the archive given on the [release page](https://github.com/baskerville/plato/releases):

```sh
mkdir -p SD_ROOT/.adds/plato
unzip plato-VERSION.zip -d SD_ROOT/.adds/plato
```

`SD_ROOT` is the (platform dependent) root of the SD card.

### Launchers

Pick one launcher:

- [kfmon](https://github.com/niluje/kfmon).
- [fmon](https://github.com/baskerville/fmon).
- [KSM 09](https://www.mobileread.com/forums/showthread.php?t=293804).

If you choose *kfmon* or *fmon*, you'll need to extract the relevant archive:
```sh
unzip plato-launcher-fmon-VERSION.zip -d SD_ROOT
```

## Configure

The default library path is `/mnt/onboard` on devices without an external SD card, and `/mnt/sd` otherwise. If your library lives somewhere else, you'll need to create a file named `Settings.toml` in the same directory as the program's binary with the following content:
```toml
library-path = "LIBRARY_PATH"
```

The default ePUB stylesheet, `css/epub.css`, can be overriden via `css/epub-user.css`.
