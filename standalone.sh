#! /bin/sh

if [ "$#" -lt 2 ] ; then
	printf "Usage: %s FIRMWARE_ARCHIVE NICKEL_MENU_ARCHIVE\n" "${0##*/}" >&2
	exit 1
fi

[ -d dist ] || ./dist.sh
[ -d standalone ] && rm -Rf standalone

FIRMWARE_ARCHIVE=$1
NICKEL_MENU_ARCHIVE=$2

mkdir standalone
cd standalone || exit 1

unzip "$FIRMWARE_ARCHIVE" KoboRoot.tgz
tar -xzvf KoboRoot.tgz etc/init.d/rcS
patch -p 1 < ../contrib/firmware.patch || exit 1
rm KoboRoot.tgz

unzip "$NICKEL_MENU_ARCHIVE" KoboRoot.tgz
tar -xzvf KoboRoot.tgz
rm KoboRoot.tgz
mv mnt/onboard/.adds .
rm -Rf mnt

mkdir -p usr/local
mkdir -p .adds/plato

mv ../dist usr/local/Plato
for name in bin css dictionaries hyphenation-patterns keyboard-layouts ; do
	mv usr/local/Plato/"$name" .adds/plato
	ln -s /mnt/onboard/.adds/plato/"$name" usr/local/Plato
done

ln -s /mnt/onboard/.adds/plato/Settings.toml usr/local/Plato/Settings.toml
cp ../contrib/NickelMenu/plato .adds/nm

mkdir .kobo
tar -czvf .kobo/KoboRoot.tgz etc usr
rm -Rf etc usr

FIRMWARE_VERSION=$(basename "$FIRMWARE_ARCHIVE" .zip)
FIRMWARE_VERSION=${FIRMWARE_VERSION##*-}
PLATO_VERSION=$(cargo pkgid | cut -d '#' -f 2)

zip -r plato-standalone-"$PLATO_VERSION"-fw_"$FIRMWARE_VERSION".zip .adds .kobo
rm -Rf .adds .kobo
