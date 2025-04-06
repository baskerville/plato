#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

PLATO_SET_FRAMEBUFFER_DEPTH=1
PLATO_CONVERT_DICTIONARIES=1

# shellcheck disable=SC1091
[ -e config.sh ] && . config.sh

# shellcheck disable=SC2046
export $(grep -sE '^(INTERFACE|WIFI_MODULE|DBUS_SESSION_BUS_ADDRESS|NICKEL_HOME|LANG)=' /proc/"$(pidof -s nickel)"/environ)
sync
killall -TERM nickel hindenburg sickel fickel adobehost foxitpdf iink dhcpcd-dbus dhcpcd fmon > /dev/null 2>&1


if [ -e /sys/class/leds/LED ] ; then
	LEDS_INTERFACE=/sys/class/leds/LED/brightness
	STANDARD_LEDS=1
elif [ -e /sys/class/leds/GLED ] ; then
	LEDS_INTERFACE=/sys/class/leds/GLED/brightness
	STANDARD_LEDS=1
elif [ -e /sys/class/leds/bd71828-green-led ] ; then
	LEDS_INTERFACE=/sys/class/leds/bd71828-green-led/brightness
	STANDARD_LEDS=1
elif [ -e /sys/devices/platform/ntx_led/lit ] ; then
	LEDS_INTERFACE=/sys/devices/platform/ntx_led/lit
	STANDARD_LEDS=0
elif [ -e /sys/devices/platform/pmic_light.1/lit ] ; then
	LEDS_INTERFACE=/sys/devices/platform/pmic_light.1/lit
	STANDARD_LEDS=0
fi

# Turn off the LEDs
if [ "$STANDARD_LEDS" -eq 1 ] ; then
	echo 0 > "$LEDS_INTERFACE"
else
	# https://www.tablix.org/~avian/blog/archives/2013/03/blinken_kindle/
	for ch in 3 4 5; do
		echo "ch ${ch}" > "$LEDS_INTERFACE"
		echo "cur 1" > "$LEDS_INTERFACE"
		echo "dc 0" > "$LEDS_INTERFACE"
	done
fi

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
		3[12]0)  PRODUCT_ID=0x4163 ;; # Touch A/B, Touch C
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
		386)     PRODUCT_ID=0x4235 ;; # Clara 2E
		389)     PRODUCT_ID=0x4236 ;; # Elipsa 2E
		390)     PRODUCT_ID=0x4237 ;; # Libra Colour
		393)     PRODUCT_ID=0x4238 ;; # Clara Colour
		391)     PRODUCT_ID=0x4239 ;; # Clara BW
		*)       PRODUCT_ID=0x6666 ;;
	esac

	export SERIAL_NUMBER FIRMWARE_VERSION MODEL_NUMBER PRODUCT_ID
fi

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"

[ -e info.log ] && [ "$(stat -c '%s' info.log)" -gt $((1<<18)) ] && mv info.log archive.log

[ "$PLATO_CONVERT_DICTIONARIES" ] && find -L dictionaries -name '*.ifo' -exec ./convert-dictionary.sh {} \;

if [ "$PLATO_SET_FRAMEBUFFER_DEPTH" ] ; then
	case "${PRODUCT}:${MODEL_NUMBER}" in
		kraken:*|pixie:*|dragon:*|phoenix:*|dahlia:*|alyssum:*|pika:*|daylight:*|star:375|snow:374)
			ORIG_BPP=$(./bin/utils/fbdepth -g)
			;;
		*)
			unset ORIG_BPP
			;;
	esac
fi

[ "$ORIG_BPP" ] && ./bin/utils/fbdepth -q -d 8

LIBC_FATAL_STDERR_=1 ./plato >> info.log 2>&1

[ "$ORIG_BPP" ] && ./bin/utils/fbdepth -q -d "$ORIG_BPP"

if [ -e /tmp/reboot ] ; then
	reboot
elif [ -e /tmp/power_off ] ; then
	poweroff -f
else
	./nickel.sh &
fi
