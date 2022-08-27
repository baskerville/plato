#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

PLATO_SET_FRAMEBUFFER_DEPTH=1
PLATO_CONVERT_DICTIONARIES=1

# shellcheck disable=SC1091
[ -e config.sh ] && . config.sh

if [ "$PLATO_STANDALONE" ] ; then
	# Stop the animation started by rcS
	REM_TRIES=10
	while [ "$REM_TRIES" -gt 0 ] ; do
		killall on-animator.sh && break
		REM_TRIES=$((REM_TRIES-1))
		usleep 400000
	done
else
	# shellcheck disable=SC2046
	export $(grep -sE '^(INTERFACE|WIFI_MODULE|DBUS_SESSION_BUS_ADDRESS|NICKEL_HOME|LANG)=' /proc/"$(pidof -s nickel)"/environ)
	sync
	killall -TERM nickel hindenburg sickel fickel adobehost foxitpdf iink dhcpcd-dbus dhcpcd fmon > /dev/null 2>&1
fi

# Turn off the LEDs
# https://www.tablix.org/~avian/blog/archives/2013/03/blinken_kindle/
LEDS_INTERFACE=/sys/devices/platform/pmic_light.1/lit
for ch in 3 4 5; do
	echo "ch ${ch}" > "$LEDS_INTERFACE"
	echo "cur 1" > "$LEDS_INTERFACE"
	echo "dc 0" > "$LEDS_INTERFACE"
done

# Remount the SD card read-write if it's mounted read-only
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
		382)     PRODUCT_ID=0x4230 ;; # Nia
		387)     PRODUCT_ID=0x4233 ;; # Elipsa
		383)     PRODUCT_ID=0x4231 ;; # Sage
		388)     PRODUCT_ID=0x4234 ;; # Libra 2
		*)       PRODUCT_ID=0x6666 ;;
	esac

	export SERIAL_NUMBER FIRMWARE_VERSION MODEL_NUMBER PRODUCT_ID
fi

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"

[ -e info.log ] && [ "$(stat -c '%s' info.log)" -gt $((1<<18)) ] && mv info.log archive.log

[ "$PLATO_CONVERT_DICTIONARIES" ] && find -L dictionaries -name '*.ifo' -exec ./convert-dictionary.sh {} \;

if [ "$PLATO_SET_FRAMEBUFFER_DEPTH" ] ; then
	case "${PRODUCT}:${MODEL_NUMBER}" in
		io:*|cadmus:*|europa:*|storm:*|frost:*|nova:*|snow:378|star:379)
			unset ORIG_BPP
			;;
		*)
			ORIG_BPP=$(./bin/utils/fbdepth -g)
			;;
	esac
fi

[ "$ORIG_BPP" ] && ./bin/utils/fbdepth -q -d 8

LIBC_FATAL_STDERR_=1 ./plato >> info.log 2>&1 || rm bootlock

[ "$ORIG_BPP" ] && ./bin/utils/fbdepth -q -d "$ORIG_BPP"

if [ -e /tmp/reboot ] ; then
	reboot
elif [ -e /tmp/power_off ] ; then
	poweroff -f
elif [ "$PLATO_STANDALONE" ] ; then
	reboot
else
	./nickel.sh &
fi
