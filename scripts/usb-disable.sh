#! /bin/sh

lsmod | grep -q g_file_storage || exit 1

rmmod g_file_storage

case "$PLATFORM" in
	mx6[su]ll-ntx)
		rmmod usb_f_mass_storage
		rmmod libcomposite
		rmmod configfs
		;;
	*)
		lsmod | grep -q arcotg_udc && rmmod arcotg_udc
		;;
esac

sleep 1

DISK=/dev/mmcblk
PARTITION=${DISK}0p3
MOUNT_ARGS="noatime,nodiratime,shortname=mixed,utf8"

dosfsck -a -w "$PARTITION"

mount -o "$MOUNT_ARGS" -t vfat "$PARTITION" /mnt/onboard

PARTITION=${DISK}1p1

[ -e "$PARTITION" ] && mount -o "$MOUNT_ARGS" -t vfat "$PARTITION" /mnt/sd
