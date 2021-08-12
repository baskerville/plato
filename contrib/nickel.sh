#! /bin/sh

export LD_LIBRARY_PATH=/usr/local/Kobo
export QT_GSTREAMER_PLAYBIN_AUDIOSINK=alsasink
export QT_GSTREAMER_PLAYBIN_AUDIOSINK_DEVICE_PARAMETER=bluealsa:DEV=00:00:00:00:00:00

(
	if [ "${PLATFORM}" = "freescale" ] || [ "${PLATFORM}" = "mx50-ntx" ] || [ "${PLATFORM}" = "mx6sl-ntx" ]; then
		usleep 400000
	fi
	/etc/init.d/on-animator.sh
) &

# Nickel wants the WiFi to be down when it starts
./scripts/wifi-disable.sh

# Reset PWD to a sane value, outside of onboard, so that USBMS behaves properly
cd /
# And clear up our own stuff from the env while we're there
unset OLDPWD SERIAL_NUMBER FIRMWARE_VERSION MODEL_NUMBER PRODUCT_ID

/usr/local/Kobo/hindenburg &
/usr/local/Kobo/nickel -platform kobo -skipFontLoad &
udevadm trigger &

# Notify Nickel of the existence of a mounted SD card
if [ -e /dev/mmcblk1p1 ]; then
	echo "sd add /dev/mmcblk1p1" > /tmp/nickel-hardware-status &
fi
