#! /bin/sh

WORKDIR=$(dirname "$0")
cd "$WORKDIR" || exit 1

export LD_LIBRARY_PATH="libs:${LD_LIBRARY_PATH}"

./plato > info.log 2>&1
EXIT_CODE=$?

[ "$EXIT_CODE" -ne 0 ] && mv info.log crash.log

exit "$EXIT_CODE"
