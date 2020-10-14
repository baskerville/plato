#! /bin/sh

export TRIPLE=arm-linux-gnueabihf

[ -x configure ] || ./autogen.sh
./configure --host="$TRIPLE" && make
