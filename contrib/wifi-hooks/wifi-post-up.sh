#!/bin/sh

# Dropbear can generate a host key automatically (-R), but the file location is
# configured at build-time and many ARM builds of dropbear have weird locations.
# In order to specify a location, we need to generate the key manually.
if [ ! -f /etc/dropbear/dropbear_ecdsa_host_key ]; then
	mkdir -p /etc/dropbear
	/mnt/onboard/.adds/bin/dropbearkey -t ecdsa -f /etc/dropbear/dropbear_ecdsa_host_key
fi

# add `-n` to skip password check, for initial user creation or password setting
/mnt/onboard/.adds/bin/dropbear -r /etc/dropbear/dropbear_ecdsa_host_key -p 2233
