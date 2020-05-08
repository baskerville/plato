#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

eval "$(xargs -n 1 -0 < /proc/"$(pidof nickel)"/environ | grep -E 'INTERFACE|WIFI_MODULE|DBUS_SESSION|NICKEL_HOME|LANG' | sed -e 's/^/export /')"
sync
killall -TERM nickel hindenburg sickel fickel fmon > /dev/null 2>&1

grep -q ' /mnt/sd .*[ ,]ro[ ,]' /proc/mounts && mount -o remount,rw /mnt/sd

# Define environment variables used by `scripts/usb-*.sh`
KOBO_TAG=/mnt/onboard/.kobo/version
if [ -e "$KOBO_TAG" ] ; then
	SERIAL_NUMBER=$(cut -f 1 -d ',' "$KOBO_TAG")
	FIRMWARE_VERSION=$(cut -f 3 -d ',' "$KOBO_TAG")
	MODEL_NUMBER=$(cut -f 6 -d ',' "$KOBO_TAG" | sed -e 's/^[0-]*//')

	# This is a combination of the information given in `FBInk/fbink_device_id.c`
	# and `calibre/src/calibre/devices/kobo/driver.py`.
	case "$MODEL_NUMBER" in
		310|320) PRODUCT_ID=0x4163 ;; # Touch A/B, Touch C
		330)     PRODUCT_ID=0x4173 ;; # Glo
		340)     PRODUCT_ID=0x4183 ;; # Mini
		350)     PRODUCT_ID=0x4193 ;; # Aura HD
		360)     PRODUCT_ID=0x4203 ;; # Aura
		370)     PRODUCT_ID=0x4213 ;; # Aura H₂O
		371)     PRODUCT_ID=0x4223 ;; # Glo HD
		372)     PRODUCT_ID=0x4224 ;; # Touch 2.0
		373|381) PRODUCT_ID=0x4225 ;; # Aura ONE, Aura ONE Limited Edition
		374)     PRODUCT_ID=0x4227 ;; # Aura H₂O Edition 2
		375)     PRODUCT_ID=0x4226 ;; # Aura Edition 2
		376)     PRODUCT_ID=0x4228 ;; # Clara HD
		377|380) PRODUCT_ID=0x4229 ;; # Forma, Forma 32GB
		384)     PRODUCT_ID=0x4232 ;; # Libra H₂O
		*)       PRODUCT_ID=0x6666 ;;
	esac

	export SERIAL_NUMBER FIRMWARE_VERSION MODEL_NUMBER PRODUCT_ID
fi

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"

[ -e info.log ] && [ "$(stat -c '%s' info.log)" -gt $((1<<18)) ] && mv info.log archive.log

ORIG_BPP=$(./bin/utils/fbdepth -g)
./bin/utils/fbdepth -d 8

RUST_BACKTRACE=1 ./plato >> info.log 2>&1

./bin/utils/fbdepth -d "$ORIG_BPP"

./nickel.sh &
