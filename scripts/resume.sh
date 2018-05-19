#! /bin/sh


[[ -f /sys/power/state-extended ]] && echo 0 > /sys/power/state-extended
# echo a > /sys/devices/virtual/input/input1/neocmd
