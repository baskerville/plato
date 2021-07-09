#! /bin/sh

lsmod | grep -q sdio_wifi_pwr || exit 1

killall udhcpc default.script wpa_supplicant 2> /dev/null

[ "$WIFI_MODULE" != 8189fs ] && [ "$WIFI_MODULE" != 8192es ] && [ "$WIFI_MODULE" != 8821cs ] && wlarm_le -i "$INTERFACE" down
ifconfig "$INTERFACE" down

sleep 0.2
rmmod "$WIFI_MODULE"
rmmod sdio_wifi_pwr
