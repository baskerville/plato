#! /bin/sh

lsmod | grep -q sdio_wifi_pwr || exit 1

wlarm_le -i eth0 down
ifconfig eth0 down
rmmod -r "$WIFI_MODULE"
rmmod -r sdio_wifi_pwr

killall udhcpc default.script wpa_supplicant dhcpcd-dbus dhcpcd 2> /dev/null
