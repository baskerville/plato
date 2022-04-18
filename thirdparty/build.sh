#! /usr/bin/env bash

set -e

declare -a packages=(zlib bzip2 libpng libjpeg openjpeg jbig2dec freetype2 harfbuzz gumbo djvulibre mupdf)

for name in "${@:-${packages[@]}}" ; do
	cd "$name"
	echo "Building ${name}."
	[ -e kobo.patch ] && patch -p 1 < kobo.patch
	./build-kobo.sh
	cd ..
done
