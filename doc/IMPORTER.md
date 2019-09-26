# Building on Linux

1. Install the following dependencies with your package manager: *DjVuLibre*, *HarfBuzz*, *OpenJPEG*, *jpeg*, *jbig2dec*, *zlib*.
2. Unpack the sources for *MuPDF 1.16.0* and copy the files in `contrib/linux/mupdf` in the same directory.
3. Apply the patch and build the library (it will appear in `build/release`):
```sh
patch -p 1 < linux.patch
./build-linux.sh
```
4. Within *Plato*'s directory, go to `src/wrapper` and build the wrapper (replace `<path_to_mupdf>` with the appropriate path):
```sh
CFLAGS="-I<path_to_mupdf>/include" ./build.sh
```
5. Copy the generated libraries (`libmupdf.so` and `libmupdfwrapper.so`) in the appropriate directory (e.g. `/usr/lib`).
6. And finally, in *Plato*'s directory (the binary is saved in `~/.cargo/bin`): 
```sh
cargo install --path . --bin plato-import --features importer
```
