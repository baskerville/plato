#! /bin/sh

FREETYPE_DIR=$(realpath ../freetype2)
BZIP2_DIR=$(realpath ../bzip2)
LIBPNG_DIR=$(realpath ../libpng)
ZLIB_DIR=$(realpath ../zlib)
export TRIPLE=arm-linux-gnueabihf
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"
export FREETYPE_CFLAGS="-I${FREETYPE_DIR}/include"
export FREETYPE_LIBS="-L${FREETYPE_DIR}/objs/.libs -L${LIBPNG_DIR}/.libs -L${BZIP2_DIR} -L${ZLIB_DIR} -lfreetype -lpng16 -lbz2 -lz"

./autogen.sh --host=${TRIPLE} --disable-static --with-freetype=yes --with-glib=no --with-icu=no --with-fontconfig=no && make
