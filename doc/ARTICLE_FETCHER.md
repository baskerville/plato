A *wallabag* article fetcher is distributed in the release archive, in `bin/article_fetcher`.

## Configuration

Rename `Settings-sample.toml` to `Settings.toml` and fill it out.

The fetcher manages a `.session.json` file that you shouldn't modify or remove.

## Usage

In the library menu:
- Select *Library → On Board*.
- Select *Toggle Select → Articles* (the downloaded articles are saved in the hook's *path*).

If the *Toggle Select* sub-menu is missing, [add the relevant hook](HOOKS.md).

## Build

The default article fetcher can be built with:

```sh
cargo +nightly build --profile release-minsized -Z build-std=std,panic_abort \
                     --target arm-unknown-linux-gnueabihf \
                     --bin article_fetcher -p fetcher
```
