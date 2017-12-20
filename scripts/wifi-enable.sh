#! /bin/sh

lsmod | grep -q sdio_wifi_pwr && exit 1

insmod /drivers/${PLATFORM}/wifi/sdio_wifi_pwr.ko
insmod "$WIFI_MODULE_PATH"

sleep 1

ifconfig $INTERFACE up
wlarm_le -i $INTERFACE up

pidof wpa_supplicant > /dev/null || wpa_supplicant -D wext -s -i $INTERFACE -c /etc/wpa_supplicant/wpa_supplicant.conf -C /var/run/wpa_supplicant -B

sleep 1

udhcpc -S -i $INTERFACE -s /etc/udhcpc.d/default.script -t15 -T10 -A3 -b -q > /dev/null 2>&1 &
