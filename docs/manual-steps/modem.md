# The modem

**State.** Nothing boots the modem under our `/init`; `balong_power`'s `state` reads
`MODEM_STATE_OFF`, and the LPM3 counts it awake in every suspend. Booted by hand from the
debug shell with its radio off, a suspend reads `modem:0`. The boot writes the
`modemnvm_*` partitions through the kernel; all eight modem partitions are backed up under
`~/p20-backups/*-2026-11-19.img`, and their roles are in
[docs/device-history/phone.md](../device-history/phone.md), *the deep-sleep votes*.
→ ADR 0012, D3.

**Reproduce**, from the debug shell in a fresh boot, before the suspend in
[phone.md](phone.md):

```sh
mount -t ext4 -o ro /dev/mmcblk0p57 /vendor
o=rw,noauto_da_alloc,nosuid,nodev,noatime,data=ordered,barrier=1
mkdir -p /mnt/modem/modem_secure /mnt/modem/mnvm2:0
mount -t ext4 -o $o /dev/mmcblk0p17 /mnt/modem/modem_secure
mount -t ext4 -o $o /dev/mmcblk0p18 /modem_log
mount -t ext4 -o ro /dev/mmcblk0p37 /vendor/modem/modem_fw
mount -t ext4 -o $o /dev/mmcblk0p11 /mnt/modem/mnvm2:0
mount -t ext4 -o rw,nosuid,nodev,noatime,discard,auto_da_alloc,data=journal \
  /dev/mmcblk0p13 /sec_storage
mount -t tmpfs tmpfs /data && mkdir -p /data/sec_storage_data /data/sec_storage_data_users
(cd / && /vendor/bin/teecd > /mnt/teecd.out 2>&1 &)
echo 1 > /sys/devices/platform/his_modem/modem_sysboot_start
exec 3<>/dev/appvcom; printf 'AT+CFUN=0\r' >&3; timeout 3 cat <&3; exec 3>&-
```

⚠️ Leave out the ROM's `context=` mount options: no SELinux policy is loaded.

⚠️ There is no `/dev/block` under the debug `/init`; name each partition as
`/dev/mmcblk0p<n>`.

⚠️ The first read of `/dev/appvcom` returns the modem's queued reports before the answer;
read until `OK`.

**Verify.**

```sh
cat /sys/devices/platform/balong_power/state   # MODEM_STATE_READY within about 6 s
exec 3<>/dev/appvcom; printf 'AT+CFUN?\r' >&3; timeout 3 cat <&3; exec 3>&-   # +CFUN: 0
grep -E 'SR:wakelock|iavg_ua' /mnt/dmesg-1      # after the suspend: modem:0
```

**Undo.** A reboot; nothing of it outlives the boot but what the modem wrote to its own
partitions.

The readings: [docs/device-history/phone.md](../device-history/phone.md), issue 70.
