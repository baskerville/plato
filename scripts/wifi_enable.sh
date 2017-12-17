#! /bin/sh

lsmod | grep -q sdio_wifi_pwr && exit 1

insmod /drivers/${PLATFORM}/wifi/sdio_wifi_pwr.ko
insmod /drivers/${PLATFORM}/wifi/${WIFI_MODULE}.ko

sleep 1

ifconfig eth0 up
wlarm_le -i eth0 up

pidof wpa_supplicant > /dev/null || wpa_supplicant -D wext -s -i eth0 -c /etc/wpa_supplicant/wpa_supplicant.conf -C /var/run/wpa_supplicant -B

sleep 1

udhcpc -S -i eth0 -s /etc/udhcpc.d/default.script -t15 -T10 -A3 -b -q > /dev/null 2>&1 &
