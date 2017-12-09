#! /bin/sh

CC=${CC:-gcc}
LD=${LD:-ld}

TARGET_OS=$(uname -s)
if [ "$TARGET_OS" = "Darwin" ] ; then
	LIB_EXT=dylib
else
	LIB_EXT=so
fi

$CC $CPPFLAGS $CFLAGS -fPIC -c mupdf.c
$CC $LDFLAGS -shared -fPIC -o libmupdfwrapper.${LIB_EXT} mupdf.o
rm mupdf.o
