#! /bin/sh

set -e

CC=${CC:-gcc}
AR=${AR:-ar}

TARGET_OS=${TARGET_OS:-$(uname -s)}
BUILD_DIR=../target/mupdf_wrapper/${TARGET_OS}
mkdir -p $BUILD_DIR
$CC $CPPFLAGS $CFLAGS -I../thirdparty/mupdf/include -c mupdf_wrapper.c -o ${BUILD_DIR}/mupdf_wrapper.o
$AR -rcs ${BUILD_DIR}/libmupdf_wrapper.a ${BUILD_DIR}/mupdf_wrapper.o
