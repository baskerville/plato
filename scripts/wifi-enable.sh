#! /bin/sh

lsmod | grep -q sdio_wifi_pwr && exit 1

insmod /drivers/${PLATFORM}/wifi/sdio_wifi_pwr.ko
insmod "$WIFI_MODULE_PATH"

while [ ! -e /sys/class/net/${INTERFACE} ] ; do
	sleep 0.2
done

ifconfig $INTERFACE up
[ "$WIFI_MODULE" != 8189fs ] && wlarm_le -i $INTERFACE up

pidof wpa_supplicant > /dev/null || wpa_supplicant -D wext -s -i $INTERFACE -c /etc/wpa_supplicant/wpa_supplicant.conf -C /var/run/wpa_supplicant -B

udhcpc -S -i $INTERFACE -s /etc/udhcpc.d/default.script -t15 -T10 -A3 -b -q > /dev/null &
