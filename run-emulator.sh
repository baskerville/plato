#! /bin/sh

if ! [ -e Settings.toml ]; then
	cat <<- EOF > Settings.toml
	[[libraries]]
	name = "Example"
	path = "$PWD"
	mode = "database"
	EOF
fi

./service.sh run_emulator "$@"
