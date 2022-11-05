#! /bin/sh

grep -q "^${WIFI_MODULE}\b" /proc/modules || exit 1

SCRIPTS_DIR=$(dirname "$0")
PRE_DOWN_SCRIPT=$SCRIPTS_DIR/wifi-pre-down.sh
[ -e "$PRE_DOWN_SCRIPT" ] && $PRE_DOWN_SCRIPT

HAS_SDIO_WIFI_PWR=1
[ "$WIFI_MODULE" = moal ] && HAS_SDIO_WIFI_PWR=0

killall -q udhcpc default.script

wpa_cli -i "$INTERFACE" terminate
[ "$WIFI_MODULE" = dhd ] && wlarm_le -i "$INTERFACE" down
ifconfig "$INTERFACE" down

sleep 0.2

rmmod "$WIFI_MODULE"

[ "$WIFI_MODULE" = moal ] && rmmod mlan

if [ "$HAS_SDIO_WIFI_PWR" -eq 1 ]; then
	rmmod sdio_wifi_pwr
else
	# CM_WIFI_CTRL
	ioctl -q -v 0 /dev/ntx_io 208
fi

POST_DOWN_SCRIPT=$SCRIPTS_DIR/wifi-post-down.sh
[ -e "$POST_DOWN_SCRIPT" ] && $POST_DOWN_SCRIPT
