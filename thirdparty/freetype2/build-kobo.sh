#! /bin/sh

export TRIPLE=arm-linux-gnueabihf
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"
export ZLIB_CFLAGS="-I../zlib"
export ZLIB_LIBS="-L../zlib -lz"
export BZIP2_CFLAGS="-I../bzip2"
export BZIP2_LIBS="-L../bzip2 -lbz2"
export LIBPNG_CFLAGS="-I../libpng"
export LIBPNG_LIBS="-L../libpng/.libs -lpng16"

./configure --host=${TRIPLE} --with-zlib=yes --with-png=yes \
            --with-bzip2=yes --with-harfbuzz=no --with-brotli=no \
            --disable-static && make
