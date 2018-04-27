## Install

Extract the archive given on the [release page](https://github.com/baskerville/plato/releases):

```sh
mkdir -p SD_ROOT/.adds/plato
unzip plato-VERSION.zip -d SD_ROOT/.adds/plato
```

`SD_ROOT` is the (platform dependent) root of the SD card.

### Launchers

#### KSM

Install [KSM](https://www.mobileread.com/forums/showthread.php?t=293804).

#### fmon

Install [fmon](https://github.com/baskerville/fmon).

And extract the relevant archive:
```sh
unzip plato-launcher-fmon-VERSION.zip -d SD_ROOT
```

*Plato*'s icon should be imported when you eject the card.
Follow the instruction given on the *fmon* page on how to handle it.

## Configure

The default library path is `/mnt/onboard`. If your library lives somewhere else, you'll need to create a file named `settings.json` in the same directory as the program's binary with the following content:
```json
{ "libraryPath": "LIBRARY_PATH" }
```

If there's a `user.css` in same directory as the program's binary, it will be used for all the reflowable formats.
