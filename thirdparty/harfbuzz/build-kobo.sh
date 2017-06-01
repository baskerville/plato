#! /bin/sh

FREETYPE_DIR=$(readlink -f ../freetype2)
export TRIPLE=arm-linux-gnueabihf
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"
export FREETYPE_CFLAGS="-I${FREETYPE_DIR}/include"
export FREETYPE_LIBS="-L${FREETYPE_DIR}/objs/.libs -lfreetype"

./configure --host=${TRIPLE} --disable-static --with-freetype=yes --with-fontconfig=no && make
