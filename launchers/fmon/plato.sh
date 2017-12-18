#! /bin/sh

# Start from our working directory
WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

# Check whether nickel is running
FROM_NICKEL=0
pgrep -f nickel > /dev/null && FROM_NICKEL=1

if [ "$FROM_NICKEL" -eq 1 ]; then
	# Siphon a few things from nickel's environment
	eval "$(xargs -n 1 -0 < /proc/$(pidof nickel)/environ | grep -e DBUS_SESSION_BUS_ADDRESS -e WIFI_MODULE -e INTERFACE)"
	export DBUS_SESSION_BUS_ADDRESS WIFI_MODULE INTERFACE
	# Flush the disks: might help avoid damaging nickel's DB...
	sync
	# Kill nickel and friends
	killall nickel hindenburg sickel fickel fmon > /dev/null 2>&1
fi

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"
export PRODUCT=$(/bin/kobo_config.sh 2> /dev/null)

./plato > info.log 2>&1
RESULT=$?

[ "$RESULT" -ne 0 ] && mv info.log crash.log

if [ "$FROM_NICKEL" -eq 1 ]; then
	# Start nickel if it was running before
	./nickel.sh &
elif ! pgrep -f kbmenu > /dev/null; then
	# If we were called from advboot then we must reboot
	/sbin/reboot
fi

return $RESULT
