#! /bin/sh

TRIPLE=arm-linux-gnueabihf
JPEG_DIR=../libjpeg
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"
export CXX=${TRIPLE}-g++
export AS=${TRIPLE}-as

./configure --host=arm-linux-gnueabihf --disable-xmltools --disable-desktopfiles --with-jpeg=${JPEG_DIR} && make
