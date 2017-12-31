#! /bin/sh

# Prevent false sleep state.
# https://github.com/koreader/koreader/commit/71afe3606ca777e4e01fcb3c9a5323cf08bdfc0c
sleep 15

# De-activate the touch screen.
echo 1 > /sys/power/state-extended

# Prevent the following error on the last line:
# *write error: Operation not permitted*.
sleep 2

# Synchronize the file system.
sync

# Suspend to RAM.
echo mem > /sys/power/state
