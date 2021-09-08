#! /bin/sh

export LD_LIBRARY_PATH=/usr/local/Kobo
export QT_GSTREAMER_PLAYBIN_AUDIOSINK=alsasink
export QT_GSTREAMER_PLAYBIN_AUDIOSINK_DEVICE_PARAMETER=bluealsa:DEV=00:00:00:00:00:00

(
	if [ "$PLATFORM" = "freescale" ] || [ "$PLATFORM" = "mx50-ntx" ] || [ "$PLATFORM" = "mx6sl-ntx" ]; then
		usleep 400000
	fi
	/etc/init.d/on-animator.sh
) &

# Let Nickel remounts the SD card read only.
[ -e /dev/mmcblk1p1 ] && umount /mnt/sd

# Nickel wants the WiFi to be down when it starts
./scripts/wifi-disable.sh

# Reset PWD to a sane value, outside of onboard, so that USBMS behaves properly
cd /
# And clear up our own stuff from the env while we're there
unset OLDPWD SERIAL_NUMBER FIRMWARE_VERSION MODEL_NUMBER PRODUCT_ID

/usr/local/Kobo/hindenburg &
LIBC_FATAL_STDERR_=1 /usr/local/Kobo/nickel -platform kobo -skipFontLoad &
[ "$PLATFORM" != "freescale" ] && udevadm trigger &
