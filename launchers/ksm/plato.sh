#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"

FRAMEBUFFER_ROTATE=/sys/class/graphics/fb0/rotate
ROTATION=$(cat "$FRAMEBUFFER_ROTATE")
echo 3 > "$FRAMEBUFFER_ROTATE"

./plato > info.log 2>&1
EXIT_CODE=$?

[ "$EXIT_CODE" -ne 0 ] && mv info.log crash.log
echo "$ROTATION" > "$FRAMEBUFFER_ROTATE"

exit "$EXIT_CODE"
