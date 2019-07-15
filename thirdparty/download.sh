#! /usr/bin/env bash

declare -A urls=(
	# Compression
	["zlib"]="https://zlib.net/zlib-1.2.11.tar.gz"
	["bzip2"]="https://ftp.osuosl.org/pub/clfs/conglomeration/bzip2/bzip2-1.0.6.tar.gz"
	# Images
	["libpng"]="https://download.sourceforge.net/libpng/libpng-1.6.37.tar.gz"
	["libjpeg"]="http://www.ijg.org/files/jpegsrc.v9c.tar.gz"
	["openjpeg"]="https://github.com/uclouvain/openjpeg/archive/v2.3.1.tar.gz"
	["jbig2dec"]="https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs927/jbig2dec-0.16.tar.gz"
	# Fonts
	["freetype2"]="https://download.savannah.gnu.org/releases/freetype/freetype-2.10.1.tar.gz"
	["harfbuzz"]="https://github.com/harfbuzz/harfbuzz/archive/2.5.3.tar.gz"
	# Documents
	["djvulibre"]="http://downloads.sourceforge.net/djvu/djvulibre-3.5.27.tar.gz"
	["mupdf"]="https://mupdf.com/downloads/archive/mupdf-1.15.0-source.tar.gz"
)

for name in "${@:-${!urls[@]}}" ; do
	echo "Downloading ${name}."
	[ -d "$name" ] || mkdir "$name"
	url="${urls[$name]}"
	wget -q -O - "$url" | tar -xz --strip-components 1 -C "$name"
done
