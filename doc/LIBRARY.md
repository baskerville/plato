## Preliminary

Install the required libraries: *mupdf* and *djvulibre*.

Then build the *mupdf* wrapper in `src/wrapper`:

```
CFLAGS='-I/path/to/mupdf/include' LDFLAGS='-lmupdf' ./build.sh
```

And put the generated library in `libs`.

## Import Metadata

The following tools will be used in the examples: [jq](https://stedolan.github.io/jq/) and [stest](https://git.suckless.org/dmenu/tree/stest.c).

First install the importer with `cargo install --path . --bin plato-import --features importer`.

Then, create an empty database with `plato-import -Z LIBRARY_PATH`.

If the command runs successfully, a file named `.metadata.json` will appear in the given directory.

The initial import is done with `plato-import -I LIBRARY_PATH`. What this does is to search for files in `LIBRARY_PATH` that aren't referenced by `.metadata.json` and save the results in `.metadata-imported.json`.

At this stage the imported metadata contains the following keys:

- `added`: the date of import.
- `file`: an object with the following keys:
	- `path`: the path of the document relative to `LIBRARY_PATH`.
	- `kind`: the lowercased file extension.
	- `size`: the file size in bytes.
- `categories`: if the document isn't a direct child of `LIBRARY_PATH`, then its relative path will be converted into a category.

The next step is to extract metadata from the ePUB documents: `plato-import -M LIBRARY_PATH`. (Subsequent commands read from **and** write to `.metadata-imported.json`.)

The final step, cleaning up, is achieved with `plato-import -C LIBRARY_PATH`.

I would recommend adding binding to your text editor to open files at the cursor position (using the double quote characters as boundary) so you can quickly fill out missing information in `.metadata-imported.json`.

## Library Synchronization

Connect your e-reader to your computer. If you're importing for the first time, create and empty database: `plato -Z EREADER_LIBRARY_PATH`. Merge the imported metadata into the e-reader's database:
```sh
cd EREADER_LIBRARY_PATH
jq -s '.|add' .metadata.json LIBRARY_PATH/.metadata-imported.json > metadata.json
mv metadata.json .metadata.json
```

Synchronize your e-reader library:

```sh
plato-import -Y LIBRARY_PATH/ EREADER_LIBRARY_PATH/`
```

Don't remove `LIBRARY_PATH/.metadata-imported.json` until all your devices are synchronized.

You can check if a database contains broken paths with:

```sh
jq -r '.[].file.path' .metadata.json | stest -ave
```
