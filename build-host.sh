#! /bin/sh

set -e

cd src/wrapper
./build.sh
cd ../..

cargo build --features emulator
