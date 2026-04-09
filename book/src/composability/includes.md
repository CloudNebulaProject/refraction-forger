# Includes (Shared Steps)

The `include` directive imports shared configuration steps from another spec file into the current spec. Unlike `base`, includes don't create a build-stage boundary — they simply pull in common definitions for reuse.

## Syntax

```kdl
include "common.kdl"
include "devfs.kdl"
```

## What Includes Do

An include file is a regular `.kdl` spec containing overlays, packages, customizations, or other configuration. When included, its contents are merged into the including spec as if they were written inline.

### Example: `common.kdl`

```kdl
overlays {
    ensure-symlink "/etc/svc/profile/generic.xml" target="generic_limited_net.xml"
    ensure-symlink "/etc/svc/profile/inetd_services.xml" target="inetd_generic.xml"
    ensure-symlink "/etc/svc/profile/platform.xml" target="platform_none.xml"
    ensure-symlink "/etc/svc/profile/name_service.xml" target="ns_dns.xml"

    file destination="/etc/inet/hosts" source="files/etc/hosts"
    file destination="/etc/nodename" source="files/etc/nodename"
    file destination="/etc/resolv.conf" source="files/etc/resolv.conf"
    ensure-symlink "/etc/nsswitch.conf" target="/etc/nsswitch.dns"
}
```

### Example: `devfs.kdl`

```kdl
overlays {
    devfsadm

    remove-files "/dev/dsk" "/dev/rdsk" "/dev/cfg" "/dev/usb"

    ensure-dir "/dev/cfg" owner="root" group="root" mode="755"
    ensure-dir "/dev/dsk" owner="root" group="root" mode="755"
    ensure-dir "/dev/rdsk" owner="root" group="root" mode="755"
    ensure-dir "/dev/usb" owner="root" group="root" mode="755"

    ensure-symlink "/dev/msglog" target="../devices/pseudo/log@0:msglog"
}
```

## Using Includes

A disk image spec can import these shared steps:

```kdl
base "omnios-base.kdl"
include "devfs.kdl"
include "common.kdl"

packages {
    package "/system/cloud-init"
}

overlays {
    file destination="/boot/conf.d/console" source="files/boot_console.115200"
    shadow username="root" password="$5$..."
}

target "vm" kind="qcow2" {
    disk-size "2G"
    bootloader "uefi"
    filesystem "zfs"
}
```

## Execution Order

1. Base spec's packages, overlays, and customizations run first
2. Include files run in the order they appear
3. The current spec's content runs last

This gives you control over layering: device filesystem setup (`devfs.kdl`) before network configuration (`common.kdl`) before image-specific overlays.

## When to Use Include vs Base

- **Include**: Shared configuration snippets used across multiple specs (SMF profiles, network setup, device nodes)
- **Base**: A full image foundation that produces a cached artifact consumed by derivative images

Includes are lightweight — they don't trigger a separate build phase or produce intermediate artifacts.
