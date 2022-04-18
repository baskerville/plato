#! /bin/sh

[ -d build ] && rm -Rf build

mkdir build
cd build || exit 1

TRIPLE=arm-linux-gnueabihf
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"

cmake -DCMAKE_BUILD_TYPE=Release -DBUILD_CODEC=off -DBUILD_STATIC_LIBS=off -DCMAKE_SYSTEM_NAME=Linux -DCMAKE_C_COMPILER=${TRIPLE}-gcc -DCMAKE_CXX_COMPILER=${TRIPLE}-g++ -DCMAKE_AR=${TRIPLE}-ar .. && make

cd .. || exit 1
cp build/src/lib/openjp2/opj_config.h src/lib/openjp2
