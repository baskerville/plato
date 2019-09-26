#! /bin/sh

[ -e thirdparty/README ] && rm -rf thirdparty/*

BUILD_KIND=${1:-release}
export CFLAGS='-O2 -fPIC -DTOFU_CJK_LANG -DTOFU_CJK_EXT -DFZ_ENABLE_ICC=0 -DFZ_ENABLE_JS=0'
make verbose=yes USE_SYSTEM_LIBS=yes "$BUILD_KIND"

gcc -Wl,--gc-sections -o build/"$BUILD_KIND"/libmupdf.so $(find build/"$BUILD_KIND" -name '*.o' | grep -Ev '(SourceHanSerif-Regular|DroidSansFallbackFull|color-lcms)') -lm -lfreetype -lharfbuzz -ljbig2dec -ljpeg -lopenjp2 -lz -shared -Wl,-soname -Wl,libmupdf.so -Wl,--no-undefined
