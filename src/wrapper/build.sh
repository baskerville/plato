#! /bin/sh

CC=${CC:-gcc}
LD=${LD:-ld}

TARGET_OS=${TARGET_OS:-$(uname -s)}
if [ "$TARGET_OS" = "Darwin" ] ; then
	LIB_EXT=dylib
else
	LIB_EXT=so
fi

mkdir -p ${TARGET_OS}
$CC $CPPFLAGS $CFLAGS -fPIC -c mupdf.c -o ${TARGET_OS}/mupdf.o
$CC $LDFLAGS -shared -fPIC -o ${TARGET_OS}/libmupdfwrapper.${LIB_EXT} ${TARGET_OS}/mupdf.o
rm ${TARGET_OS}/mupdf.o
