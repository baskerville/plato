#! /bin/sh

export LD_LIBRARY_PATH=/usr/local/Kobo

( usleep 400000; /etc/init.d/on-animator.sh ) &

# Nickel wants the WiFi to be down when it starts
./scripts/wifi-disable.sh

# Reset PWD to a sane value, outside of onboard, so that USBMS behaves properly
cd /
unset OLDPWD LC_ALL

/usr/local/Kobo/hindenburg &
/usr/local/Kobo/nickel -platform kobo -skipFontLoad &
udevadm trigger &
