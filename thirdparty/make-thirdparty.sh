cd bzip2
./build-kobo.sh
ln -s libbz2.so.1.0 libbz2.so
cd ..

cd djvulibre
./build-kobo.sh
cd ..

cd zlib
./build-kobo.sh
cd ..

cd libpng
./build-kobo.sh
cd ..

cd libjpeg
./build-kobo.sh
cd ..

cd jbig2dec
./build-kobo.sh
cd ..

cd freetype2
./build-kobo.sh no
cd ..

cd harfbuzz
./build-kobo.sh
cd ..

cd freetype2-hb
./build-kobo.sh
cd ..

cd mupdf
patch < patch.diff
sed '/OPJ_STATIC$/d' -i source/fitz/load-jpx.c
./build-kobo.sh
./make-shared-lib.sh

cd ..
cd ../src/wrapper
./build-kobo.sh

cd ../..
mkdir libs
find . -name *.so -exec cp {} libs \;

#cargo clean
./build.sh
