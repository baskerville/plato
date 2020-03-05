#! /bin/sh

if ! [ -e Settings.toml ]; then
   echo "library-path = \"${PWD}\"" > Settings.toml
   echo "[]" > .metadata.json
fi

./service.sh run_emulator "$@"
