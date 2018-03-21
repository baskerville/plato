#! /bin/sh

FREETYPE_DIR=$(readlink -f ../freetype2)
BZIP2_DIR=$(readlink -f ../bzip2)
LIBPNG_DIR=$(readlink -f ../libpng)
ZLIB_DIR=$(readlink -f ../zlib)
export TRIPLE=arm-linux-gnueabihf
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"
export FREETYPE_CFLAGS="-I${FREETYPE_DIR}/include"
export FREETYPE_LIBS="-L${FREETYPE_DIR}/objs/.libs -L${LIBPNG_DIR}/.libs -L${BZIP2_DIR} -L${ZLIB_DIR} -lfreetype -lpng -lbz2 -lz"
./autogen.sh --host=${TRIPLE} --disable-static --with-icu=no --with-freetype=yes --with-fontconfig=no && make
