## Preliminary

Install the required libraries: *mupdf* and *djvulibre*.

Then build the *mupdf* wrapper in `src/wrapper`:

```
CFLAGS='-I/path/to/mupdf/include' LDFLAGS='-lmupdf' ./build.sh
```

And put the generated library in `libs`.

## Import Metadata

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

Connect your e-reader to your computer. If you're importing for the first time, create and empty database: `plato -Z EREADER_LIBRARY_PATH`. You can then synchronize you device with:
```sh
plato-import -G LIBRARY_PATH EREADER_LIBRARY_PATH`
plato-import -Y LIBRARY_PATH EREADER_LIBRARY_PATH`
```

Once you've synchronized all your devices, you might update the local library with `plato-import -G LIBRARY_PATH`.
