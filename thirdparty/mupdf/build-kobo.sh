#! /bin/sh

[ -e thirdparty/README ] && rm -rf thirdparty/*
[ -e .gitattributes ] && rm .git*

BUILD_KIND=${1:-release}
make verbose=yes generate
make verbose=yes tesseract=no USE_SYSTEM_LIBS=yes OS=kobo build="$BUILD_KIND" libs

arm-linux-gnueabihf-gcc -Wl,--gc-sections -o build/"$BUILD_KIND"/libmupdf.so $(find build/"$BUILD_KIND" -name '*.o' | grep -Ev '(SourceHanSerif-Regular|DroidSansFallbackFull|NotoSerifTangut|color-lcms)') -lm -L../freetype2/objs/.libs -lfreetype -L../harfbuzz/src/.libs -lgumbo -L../gumbo/.libs -lharfbuzz -L../jbig2dec/.libs -ljbig2dec -L../libjpeg/.libs -ljpeg -L../openjpeg/build/bin -lopenjp2 -L../zlib -lz -shared -Wl,-soname -Wl,libmupdf.so -Wl,--no-undefined
