#! /bin/sh

lsmod | grep -q sdio_wifi_pwr || exit 1

SCRIPTS_DIR=$(dirname "$(realpath "$0")")
PRE_DOWN_SCRIPT=$SCRIPTS_DIR/wifi-pre-down.sh
[ -f "$PRE_DOWN_SCRIPT" ] && $PRE_DOWN_SCRIPT

killall udhcpc default.script wpa_supplicant 2> /dev/null

[ "$WIFI_MODULE" != 8189fs ] && [ "$WIFI_MODULE" != 8192es ] && wlarm_le -i "$INTERFACE" down
ifconfig "$INTERFACE" down

sleep 0.2
rmmod "$WIFI_MODULE"
rmmod sdio_wifi_pwr

POST_DOWN_SCRIPT=$SCRIPTS_DIR/wifi-post-down.sh
[ -f "$POST_DOWN_SCRIPT" ] && $POST_DOWN_SCRIPT
