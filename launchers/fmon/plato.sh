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

	# Taken from `KSM09/adds/kbmenu/onstart/ksmhome.sh`
	case "$MODEL_NUMBER" in
		310|320) PRODUCT_ID=0x4163 ;;
		330) PRODUCT_ID=0x4173 ;;
		340) PRODUCT_ID=0x4183 ;;
		350) PRODUCT_ID=0x4193 ;;
		360) PRODUCT_ID=0x4203 ;;
		370) PRODUCT_ID=0x4213 ;;
		371) PRODUCT_ID=0x4223 ;;
		372) PRODUCT_ID=0x4224 ;;
		373) PRODUCT_ID=0x4225 ;;
		374) PRODUCT_ID=0x4227 ;;
		375) PRODUCT_ID=0x4226 ;;
		376) PRODUCT_ID=0x4228 ;;
		377) PRODUCT_ID=0x4229 ;;
		381) PRODUCT_ID=0x4225 ;;
		*) PRODUCT_ID=0x6666 ;;
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
