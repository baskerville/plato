#! /bin/sh

sync
echo 1 > /sys/power/state-extended
[ "$PRODUCT" == phoenix ] && sleep 2
echo mem > /sys/power/state
