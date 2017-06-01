#! /bin/bash

make generate
make OS=kobo
arm-linux-gnueabihf-ar -d build/release/libmupdf.a DroidSansFallbackFull.o SourceHanSans{CN,JP,KR,TW}-Regular.o
