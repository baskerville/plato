#! /bin/sh

set -e

method=${*:-"fast"}

[ -d libs ] && method=skip

case "$method" in
	fast)
		version=$(cargo pkgid | cut -d '#' -f 2)
		archive="plato-${version}.zip"
		info_url="https://github.com/baskerville/plato/releases/tag/${version}"
		echo "Downloading ${archive}."
		release_url=$(wget -q -O - "$info_url" | grep -Eo "https[^\"]+files[^\"]+${archive}")
		wget -q "$release_url"
		unzip "$archive" 'libs/*'
		rm "$archive"
		cd libs
		
		ln -s libz.so.1 libz.so
		ln -s libbz2.so.1.0 libbz2.so

		ln -s libpng16.so.16 libpng16.so
		ln -s libjpeg.so.9 libjpeg.so
		ln -s libopenjp2.so.7 libopenjp2.so
		ln -s libjbig2dec.so.0 libjbig2dec.so

		ln -s libfreetype.so.6 libfreetype.so
		ln -s libharfbuzz.so.0 libharfbuzz.so

		ln -s libdjvulibre.so.21 libdjvulibre.so
		;;
	slow)
		cd thirdparty
		./download.sh
		./build.sh
		cd ..
		cd src/wrapper
		./build-kobo.sh
		cd ../..

		mkdir libs

		cp thirdparty/zlib/libz.so libs
		cp thirdparty/bzip2/libbz2.so libs

		cp thirdparty/libpng/.libs/libpng16.so libs
		cp thirdparty/libjpeg/.libs/libjpeg.so libs
		cp thirdparty/openjpeg/build/bin/libopenjp2.so libs
		cp thirdparty/jbig2dec/.libs/libjbig2dec.so libs

		cp thirdparty/freetype2/objs/.libs/libfreetype.so libs
		cp thirdparty/harfbuzz/src/.libs/libharfbuzz.so libs

		cp thirdparty/djvulibre/libdjvu/.libs/libdjvulibre.so libs
		cp thirdparty/mupdf/build/release/libmupdf.so libs
		cp src/wrapper/libmupdfwrapper.so libs
		;;

	skip)
		;;
	*)
		printf "Unknown build method: %s.\n" "$method" 1>&2
		exit 1
		;;
esac

cargo build --release --target=arm-unknown-linux-gnueabihf
