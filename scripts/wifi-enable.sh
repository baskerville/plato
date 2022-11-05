#! /bin/sh

grep -q "^${WIFI_MODULE}\b" /proc/modules && exit 1

SCRIPTS_DIR=$(dirname "$0")
PRE_UP_SCRIPT=$SCRIPTS_DIR/wifi-pre-up.sh
[ -e "$PRE_UP_SCRIPT" ] && $PRE_UP_SCRIPT

HAS_SDIO_WIFI_PWR=1
WPA_SUPPLICANT_DRIVER=wext

if [ "$WIFI_MODULE" = moal ]; then
	HAS_SDIO_WIFI_PWR=0
	WPA_SUPPLICANT_DRIVER=nl80211
fi

if [ "$HAS_SDIO_WIFI_PWR" -eq 1 ]; then
	insmod /drivers/"$PLATFORM"/wifi/sdio_wifi_pwr.ko
else
	# CM_WIFI_CTRL
	ioctl -q -v 1 /dev/ntx_io 208
fi

COUNTRY_CODE=$(grep "^WifiRegulatoryDomain=" "/mnt/onboard/.kobo/Kobo/Kobo eReader.conf")
if [ "$COUNTRY_CODE" ]; then
	case "$WIFI_MODULE" in
		8821cs)
			MODULE_PARAMETERS="${MODULE_PARAMETERS} rtw_country_code=${COUNTRY_CODE#*=}"
			;;
		moal)
			MODULE_PARAMETERS="${MODULE_PARAMETERS} reg_alpha2=${COUNTRY_CODE#*=}"
			;;
	esac
fi

if [ "$WIFI_MODULE" = moal ]; then
	WIFI_DEP_MODULE=mlan
	MODULE_PARAMETERS="${MODULE_PARAMETERS} mod_para=nxp/wifi_mod_para_sd8987.conf"
	if [ -e /drivers/"${PLATFORM}/${WIFI_DEP_MODULE}".ko ]; then
		insmod /drivers/"${PLATFORM}/${WIFI_DEP_MODULE}".ko
	else
		insmod /drivers/"$PLATFORM"/wifi/"$WIFI_DEP_MODULE".ko
	fi
fi

if [ -e /drivers/"${PLATFORM}/${WIFI_MODULE}".ko ]; then
	# shellcheck disable=SC2086
	insmod /drivers/"${PLATFORM}/${WIFI_MODULE}".ko$MODULE_PARAMETERS
else
	# shellcheck disable=SC2086
	insmod /drivers/"$PLATFORM"/wifi/"$WIFI_MODULE".ko$MODULE_PARAMETERS
fi

REM_TRIES=20
while [ "$REM_TRIES" -gt 0 ] ; do
	[ -e /sys/class/net/"$INTERFACE" ] && break
	REM_TRIES=$((REM_TRIES-1))
	sleep 0.2
done

ifconfig "$INTERFACE" up
[ "$WIFI_MODULE" = dhd ] && wlarm_le -i "$INTERFACE" up

pidof wpa_supplicant > /dev/null || env -u LD_LIBRARY_PATH \
	wpa_supplicant -D "$WPA_SUPPLICANT_DRIVER" -s -i "$INTERFACE" -c /etc/wpa_supplicant/wpa_supplicant.conf -C /var/run/wpa_supplicant -B

env -u LD_LIBRARY_PATH \
	udhcpc -S -i "$INTERFACE" -s /etc/udhcpc.d/default.script -t15 -T10 -A3 -b -q > /dev/null &

POST_UP_SCRIPT=$SCRIPTS_DIR/wifi-post-up.sh
[ -e "$POST_UP_SCRIPT" ] && $POST_UP_SCRIPT
