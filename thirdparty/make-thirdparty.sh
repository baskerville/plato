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
cd .libs
ln -s libpng16.so libpng.so
cd ../..
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
cd freetype2
./build-kobo.sh
cd ..