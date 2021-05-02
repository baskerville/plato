#! /usr/bin/env bash

declare -A urls=(
	# Compression
	["zlib"]="https://zlib.net/zlib-1.2.11.tar.gz"
	["bzip2"]="ftp://sourceware.org/pub/bzip2/bzip2-1.0.8.tar.gz"
	# Images
	["libpng"]="https://download.sourceforge.net/libpng/libpng-1.6.37.tar.gz"
	["libjpeg"]="http://www.ijg.org/files/jpegsrc.v9d.tar.gz"
	["openjpeg"]="https://github.com/uclouvain/openjpeg/archive/v2.3.1.tar.gz"
	["jbig2dec"]="https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs9533/jbig2dec-0.19.tar.gz"
	# Fonts
	["freetype2"]="https://download.savannah.gnu.org/releases/freetype/freetype-2.10.4.tar.gz"
	["harfbuzz"]="https://github.com/harfbuzz/harfbuzz/archive/2.8.0.tar.gz"
	# Documents
	["gumbo"]="https://github.com/google/gumbo-parser/archive/v0.10.1.tar.gz"
	["djvulibre"]="http://downloads.sourceforge.net/djvu/djvulibre-3.5.28.tar.gz"
	["mupdf"]="https://mupdf.com/downloads/archive/mupdf-1.18.0-source.tar.gz"
)

for name in "${@:-${!urls[@]}}" ; do
	echo "Downloading ${name}."
	if [ -d "$name" ]; then
		git ls-files -o --directory -z "$name" | xargs -0 rm -rf
	else
		mkdir "$name"
	fi
	url="${urls[$name]}"
	wget -q --show-progress -O "${name}.tgz" "$url"
	tar -xz --strip-components 1 -C "$name" -f "${name}.tgz" && rm "${name}.tgz"
done
