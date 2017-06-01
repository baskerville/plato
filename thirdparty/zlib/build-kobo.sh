#! /bin/sh

export CROSS_PREFIX=arm-linux-gnueabihf-
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"

./configure && make
