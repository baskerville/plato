## Applying the Firmware Patch

	unzip kobo-update-FW_VERSION.zip KoboRoot.tgz
	tar -xzvf KoboRoot.tgz etc/init.d/rcS
	patch -p 1 < firmware.patch
