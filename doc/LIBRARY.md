## Preliminary

Install the required libraries: *mupdf* and *djvulibre*.

Then build the *mupdf* wrapper in `src/wrapper`:

```
CFLAGS='-I/path/to/mupdf/include' LDFLAGS='-lmupdf' ./build
```

And put the generated library in `libs`.

## Import Metadata

The following tools will be used in the examples: [jq](https://stedolan.github.io/jq/), [rsync](https://rsync.samba.org/) and [stest](https://git.suckless.org/dmenu/tree/stest.c).

First build the importer with `cargo build --release --bin plato-import --features importer`. (The resulting binary is in `./target/release`.)

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

The next step is to extract ISBN from the documents: `plato-import -S LIBRARY_PATH`. (Subsequent commands read **and** write to `.metadata-imported.json`.)

This task might fail if:

- The document doesn't have an OCR text layer.
- The ISBN isn't given or the ISBN is listed *after* the first ten pages.
- The document predates the invention of the ISBN (1970).
- The ISBN is listed in the first ten pages but the OCR text layer is scrambled.

And then we'll try to retrieve information for each book: `plato-import -R LIBRARY_PATH`. This tasks normally uses the *ISBN* extracted earlier as input for sending a request to a server. But if the *ISBN* is missing it will use a cleaned up version of the file name as input unless `-s` is passed.
 
The final step, cleaning up, is achieved with `plato-import -C LIBRARY_PATH`.

I would recommend adding binding to your text editor to open files at the cursor position (using the double quote characters as boundary) so you can quickly fill out missing information in `.metadata-imported.json`.

## Library Synchronization

Now you can merge the imported metadata:

```sh
jq -s '.|add' .metadata.json .metadata-imported.json > metadata.json && mv metadata.json .metadata.json
```

Connect your e-reader to your computer. If you're importing for the first time, create and empty database: `plato -Z EREADER_LIBRARY_PATH`. Merge the imported metadata into the e-reader's database.

Synchronize your e-reader library with:

```sh
rsync -vurt --delete --modify-window 1 --exclude .metadata.json LIBRARY_PATH/ EREADER_LIBRARY_PATH/`
```

(Passing `--modify-window 1` is mandatory when dealing with FAT32 file systems.)

Don't remove `.metadata-imported.json` until all your devices are synchronized.

You can check if a database contains broken paths with:

```sh
jq -r '.[].file.path' .metadata.json | stest -ave
```
