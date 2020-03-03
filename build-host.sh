#! /bin/sh

set -e

cd src/mupdf_wrapper
./build.sh
cd ../..

cargo build --features emulator
