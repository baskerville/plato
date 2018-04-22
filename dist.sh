rm -rf dist
mkdir dist
cp target/arm-unknown-linux-gnueabihf/release/plato dist
arm-linux-gnueabihf-strip -s dist/plato
cp -R libs dist
mv dist/libs/libbz2.so dist/libs/libbz2.so.1.0
mv dist/libs/libdjvulibre.so dist/libs/libdjvulibre.so.21
mv dist/libs/libfreetype.so dist/libs/libfreetype.so.6
mv dist/libs/libharfbuzz.so dist/libs/libharfbuzz.so.0
mv dist/libs/libjbig2dec.so dist/libs/libjbig2dec.so.0
mv dist/libs/libjpeg.so dist/libs/libjpeg.so.9
mv dist/libs/libopenjp2.so dist/libs/libopenjp2.so.7
mv dist/libs/libpng16.so dist/libs/libpng16.so.16
mv dist/libs/libz.so dist/libs/libz.so.1
cp -R css dist
cp -R fonts dist
cp -R icons dist
cp -R scripts dist
