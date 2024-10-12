#! /usr/bin/env bash

declare -A urls=(
	# Compression
	["zlib"]="https://www.zlib.net/zlib-1.3.1.tar.gz"
	["bzip2"]="https://sourceware.org/pub/bzip2/bzip2-1.0.8.tar.gz"
	# Images
	["libpng"]="https://download.sourceforge.net/libpng/libpng-1.6.43.tar.gz"
	["libjpeg"]="http://www.ijg.org/files/jpegsrc.v9f.tar.gz"
	["openjpeg"]="https://github.com/uclouvain/openjpeg/archive/v2.5.2.tar.gz"
	["jbig2dec"]="https://github.com/ArtifexSoftware/jbig2dec/releases/download/0.20/jbig2dec-0.20.tar.gz"
	# Fonts
	["freetype2"]="https://download.savannah.gnu.org/releases/freetype/freetype-2.13.2.tar.gz"
	["harfbuzz"]="https://github.com/harfbuzz/harfbuzz/archive/8.4.0.tar.gz"
	# Documents
	["gumbo"]="https://github.com/google/gumbo-parser/archive/v0.10.1.tar.gz"
	["djvulibre"]="http://downloads.sourceforge.net/djvu/djvulibre-3.5.28.tar.gz"
	["mupdf"]="https://mupdf.com/downloads/archive/mupdf-1.23.11-source.tar.gz"
)

for name in "${@:-${!urls[@]}}" ; do
	url="${urls[$name]}"
	if [ ! "$url" ] ; then
		echo "Unknown library: ${name}." 1>&2
		exit 1
	fi
	echo "Downloading ${name}."
	if [ -d "$name" ]; then
		git ls-files -o --directory -z "$name" | xargs -0 rm -rf
	else
		mkdir "$name"
	fi
	wget -q --show-progress -O "${name}.tgz" "$url"
	tar -xz --strip-components 1 -C "$name" -f "${name}.tgz" && rm "${name}.tgz"
done
