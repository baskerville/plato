#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR"

# Stop the animation started by rcS
while true ; do
	usleep 400000
	killall on-animator.sh && break
done

# Turn off the blinking LEDs
# https://www.tablix.org/~avian/blog/archives/2013/03/blinken_kindle/
LEDS_INTERFACE=/sys/devices/platform/pmic_light.1/lit
echo "ch 4" > "$LEDS_INTERFACE"
echo "cur 0" > "$LEDS_INTERFACE"
echo "dc 0" > "$LEDS_INTERFACE"

# Define environment variables used by
# /usr/local/Kobo/udev/usb
KOBO_TAG=/mnt/onboard/.kobo/version
if [ -e "$KOBO_TAG" ] ; then
	SERIAL_NUMBER=$(cut -f 1 -d ',' "$KOBO_TAG")
	FIRMWARE_VERSION=$(cut -f 3 -d ',' "$KOBO_TAG")
	MODEL_NUMBER=$(cut -f 6 -d ',' "$KOBO_TAG" | sed -e 's/^[0-]*//')
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
		*) PRODUCT_ID=0x6666 ;;
	esac
	export SERIAL_NUMBER FIRMWARE_VERSION MODEL_NUMBER PRODUCT_ID
fi

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"
export PLATO_STANDALONE=1

LIBC_FATAL_STDERR_=1 ./plato > info.log 2>&1

# Deactivate ourselves if we crashed
if [ $? -ne 0 ] ; then
	rm bootlock
	mv info.log crash.log
fi

sync

if [ -e poweroff ] ; then
	rm poweroff
	poweroff
else
	reboot
fi
