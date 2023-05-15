#! /bin/sh

if [ $# -lt 1 ]; then
	printf 'Usage: %s CMD [OPTS].\n' "${0##*/}" 1>&2
	exit 1
fi

if ! [ -e thirdparty/mupdf/include ]; then
	cd thirdparty || exit 1
	./download.sh mupdf
	cd -
fi

WRAPPER_PATH=mupdf_wrapper
TARGET_OS=$(uname -s)

if ! [ -e "${WRAPPER_PATH}/${TARGET_OS}" ]; then
	cd "$WRAPPER_PATH" || exit 1
	./build.sh
	cd -
fi

CMD=$1
shift

case "$CMD" in
	run_emulator)
		cargo run -p emulator "$@"
		;;
	install_importer)
		cargo install --path crates/importer "$@"
		;;
	*)
		printf 'Unknown command: %s.\n' "$CMD" 1>&2
		exit 1
		;;
esac
