#! /bin/sh

LOADED_MODULE=$(grep -oE '^g_(file|mass)_storage\b' /proc/modules)

case "$LOADED_MODULE" in
	g_file_storage)
		rmmod g_file_storage

		case "$PLATFORM" in
			mx6[su]ll-ntx)
				rmmod usb_f_mass_storage
				rmmod libcomposite
				rmmod configfs
				;;
			*)
				[ "$PLATFORM" != mx6sl-ntx ] && rmmod arcotg_udc
				;;
		esac
		;;
	g_mass_storage)
		rmmod g_mass_storage
		;;
	*)
		exit 1
		;;
esac

sleep 1

DISK=/dev/mmcblk
PARTITION=${DISK}0p3
MOUNT_ARGS="noatime,nodiratime,shortname=mixed,utf8"

FS_CORRUPT=0
dosfsck -a -w "$PARTITION" || dosfsck -a -w "$PARTITION" || FS_CORRUPT=1
[ "$FS_CORRUPT" -eq 1 ] && reboot

mount -o "$MOUNT_ARGS" -t vfat "$PARTITION" /mnt/onboard

PARTITION=${DISK}1p1

[ -e "$PARTITION" ] && mount -o "$MOUNT_ARGS" -t vfat "$PARTITION" /mnt/sd
