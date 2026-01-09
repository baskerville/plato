#! /bin/sh

meson setup -Dglib=disabled -Dicu=disabled -Dcairo=disabled -Dfreetype=enabled --cross-file kobo-options.txt build
meson compile -C build
