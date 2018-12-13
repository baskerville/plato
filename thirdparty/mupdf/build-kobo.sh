#! /bin/sh

[ -e thirdparty/README ] && rm -rf thirdparty/*

BUILD_KIND=${1:-release}
make verbose=yes generate
make verbose=yes OS=kobo "$BUILD_KIND"

arm-linux-gnueabihf-gcc -Wl,--gc-sections -o build/"$BUILD_KIND"/libmupdf.so $(find build/"$BUILD_KIND" -name '*.o' | grep -Ev '(/scripts/|SourceHanSans|DroidSansFallbackFull|color-lcms)') -lm -L../freetype2/objs/.libs -lfreetype -L../harfbuzz/src/.libs -lharfbuzz -L../jbig2dec/.libs -ljbig2dec -L../libjpeg/.libs -ljpeg -L../openjpeg/build/bin -lopenjp2 -L../zlib -lz -shared -Wl,-soname -Wl,libmupdf.so -Wl,--no-undefined
