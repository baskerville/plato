#! /bin/sh

git revert --no-edit 3ed5858902055d3500a6ab183f1395686921d026 > /dev/null
[ -d build ] || mkdir build
cd build

TRIPLE=arm-linux-gnueabihf
export CFLAGS="-O2 -mcpu=cortex-a9 -mfpu=neon"
export CXXFLAGS="$CFLAGS"

cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=${TRIPLE}-gcc -DCMAKE_CXX_COMPILER=${TRIPLE}-g++ -DCMAKE_AR=${TRIPLE}-ar .. && make

cd ..
cp build/src/lib/openjp2/opj_config.h src/lib/openjp2/opj_config.h
