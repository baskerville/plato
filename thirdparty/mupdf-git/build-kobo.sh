#! /bin/sh

[ -e thirdparty/README ] && rm -rf thirdparty/*

make verbose=yes generate
make verbose=yes USE_SYSTEM_LIBS=yes OS=kobo

arm-linux-gnueabihf-gcc -Wl,--gc-sections -Wl,-s -o build/release/libmupdf.so $(find build/release -name '*.o' | grep -Ev '(SourceHanSerif-Regular|DroidSansFallbackFull|color-lcms)') -lm -L../freetype2/objs/.libs -lfreetype -L../harfbuzz/src/.libs -lharfbuzz -L../jbig2dec/.libs -ljbig2dec -L../libjpeg/.libs -ljpeg -L../openjpeg/build/bin -lopenjp2 -L../zlib -lz -shared -Wl,-soname -Wl,libmupdf.so -Wl,--no-undefined
