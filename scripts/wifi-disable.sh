#! /bin/sh

POWER_TOGGLE=module
case "$WIFI_MODULE" in
	moal)
		POWER_TOGGLE=ntx_io
		;;
	wlan_drv_gen4m)
		POWER_TOGGLE=wmt
		;;
esac

if [ "$POWER_TOGGLE" != wmt ]; then
	grep -q "^${WIFI_MODULE}\b" /proc/modules || exit 1
fi

SCRIPTS_DIR=$(dirname "$0")
PRE_DOWN_SCRIPT=$SCRIPTS_DIR/wifi-pre-down.sh
[ -e "$PRE_DOWN_SCRIPT" ] && $PRE_DOWN_SCRIPT

killall -q udhcpc default.script

wpa_cli -i "$INTERFACE" terminate
[ "$WIFI_MODULE" = dhd ] && wlarm_le -i "$INTERFACE" down
ifconfig "$INTERFACE" down

sleep 0.2

[ "$POWER_TOGGLE" != wmt ] && rmmod "$WIFI_MODULE"
[ "$WIFI_MODULE" = moal ] && rmmod mlan

case "$POWER_TOGGLE" in
	ntx_io)
		# CM_WIFI_CTRL
		ioctl -q -v 0 /dev/ntx_io 208
		;;
	wmt)
		echo 0 > /dev/wmtWifi
		;;
	module)
		rmmod sdio_wifi_pwr
		;;
esac


POST_DOWN_SCRIPT=$SCRIPTS_DIR/wifi-post-down.sh
[ -e "$POST_DOWN_SCRIPT" ] && $POST_DOWN_SCRIPT
