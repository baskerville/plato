#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

eval "$(xargs -n 1 -0 < /proc/"$(pidof nickel)"/environ | grep -E 'INTERFACE|WIFI_MODULE|DBUS_SESSION|NICKEL_HOME|LANG' | sed -e 's/^/export /')"
sync
killall -TERM nickel hindenburg sickel fickel fmon > /dev/null 2>&1

grep -q ' /mnt/sd .*[ ,]ro[ ,]' /proc/mounts && mount -o remount,rw /mnt/sd

MODEL_NUMBER=$(cut -f 6 -d ',' /mnt/onboard/.kobo/version | sed -e 's/^[0-]*//')
export MODEL_NUMBER
export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"

[ -e info.log ] && [ "$(stat -c '%s' info.log)" -gt $((1<<18)) ] && mv info.log archive.log

ORIG_BPP=$(./bin/utils/fbdepth -g)
./bin/utils/fbdepth -d 8

RUST_BACKTRACE=1 ./plato >> info.log 2>&1

./bin/utils/fbdepth -d "$ORIG_BPP"

./nickel.sh &
