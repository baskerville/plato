#! /bin/sh

lsmod | grep -q sdio_wifi_pwr || exit 1

killall udhcpc default.script wpa_supplicant 2> /dev/null

[ "$WIFI_MODULE" != 8189fs ] && wlarm_le -i "$INTERFACE" down
ifconfig "$INTERFACE" down

sleep 0.2
rmmod -r "$WIFI_MODULE"
rmmod -r sdio_wifi_pwr
