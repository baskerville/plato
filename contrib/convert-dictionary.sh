#! /bin/sh
# Converts a StarDict dictionary to the dictd format.
# The first argument must be the path to the IFO file.

trap 'exit 1' ERR

base=${1%.*}
bindir=bin/utils
short_name=$(grep '^bookname=' "$1" | cut -d '=' -f 2)
url=$(grep '^website=' "$1" | cut -d '=' -f 2)

echo "Converting ${short_name} (${1})."

[ -e "${base}.dict.dz" ] && "$bindir"/dictzip -d "${base}.dict.dz"

args="${base}.dict"

[ -e "${base}.syn" ] && args="$args ${base}.syn"

# shellcheck disable=SC2086
"$bindir"/sdunpack $args < "${base}.idx" > "${base}.txt"
[ "${short_name%% *}" = "Wiktionary" ] && sed -i 's/^\([\[/].*\)/<p>\1<\/p>/' "${base}.txt"
"$bindir"/dictfmt --quiet --utf8 --index-keep-orig --headword-separator '|' -s "$short_name" -u "$url" -t "$base" < "${base}.txt"
"$bindir"/dictzip "${base}.dict"

rm "$1" "${base}.idx" "${base}.txt"
[ -e "${base}.syn" ] && rm "${base}.syn"
