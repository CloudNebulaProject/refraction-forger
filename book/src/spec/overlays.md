# Overlays

Overlays let you place files, create directories, manage symlinks, and configure system state in the image. They run after package installation.

## File Overlay

Copy a local file into the image:

```kdl
overlays {
    file destination="/etc/ssh/sshd_config" source="files/etc/sshd_config"
}
```

| Property | Required | Description |
|---|---|---|
| `destination` | Yes | Absolute path in the image |
| `source` | Yes | Local file path (relative to spec file) |

## Ensure Directory

Create a directory with specific ownership and permissions:

```kdl
overlays {
    ensure-dir "/opt/myapp" owner="root" group="root" mode="755"
}
```

| Property | Required | Description |
|---|---|---|
| *(argument)* | Yes | Directory path in the image |
| `owner` | No | Owner user |
| `group` | No | Owner group |
| `mode` | No | Octal permissions |

## Ensure Symlink

Create a symbolic link:

```kdl
overlays {
    ensure-symlink "/etc/nsswitch.conf" target="/etc/nsswitch.dns"
}
```

| Property | Required | Description |
|---|---|---|
| *(argument)* | Yes | Symlink path in the image |
| `target` | Yes | What the symlink points to |

This is commonly used for SMF profiles on illumos:

```kdl
overlays {
    ensure-symlink "/etc/svc/profile/generic.xml" target="generic_limited_net.xml"
    ensure-symlink "/etc/svc/profile/inetd_services.xml" target="inetd_generic.xml"
    ensure-symlink "/etc/svc/profile/platform.xml" target="platform_none.xml"
    ensure-symlink "/etc/svc/profile/name_service.xml" target="ns_dns.xml"
}
```

## Remove Files

Remove files or directories from the image:

```kdl
overlays {
    remove-files "/dev/dsk" "/dev/rdsk" "/dev/cfg" "/dev/usb"
}
```

Takes one or more path arguments. Useful for cleaning up device nodes before recreating them with `devfsadm`.

## Shadow Password

Set a password hash for a user:

```kdl
overlays {
    shadow username="root" password="$5$rounds=10000$hashedvalue"
}
```

| Property | Required | Description |
|---|---|---|
| `username` | Yes | User whose password to set |
| `password` | Yes | Password hash (crypt format) |

## Device Filesystem (devfsadm)

Run `devfsadm` to populate `/dev` with device nodes. This is essential for bootable illumos images:

```kdl
overlays {
    devfsadm
}
```

This is typically used in a dedicated `devfs.kdl` include file alongside directory creation and cleanup.

## Conditional Overlays

Like packages, overlay blocks can be scoped to profiles:

```kdl
overlays if="debug" {
    file destination="/etc/system" source="files/etc/system.debug"
}
```

## Execution Order

Overlays execute in the order they appear in the spec. When multiple spec files are composed (base + includes), overlays from the base run first, followed by includes in order, then the current spec.

## Complete Example

```kdl
overlays {
    // Clean up device directories
    remove-files "/dev/dsk" "/dev/rdsk" "/dev/cfg" "/dev/usb"

    // Recreate them
    ensure-dir "/dev/cfg" owner="root" group="root" mode="755"
    ensure-dir "/dev/dsk" owner="root" group="root" mode="755"
    ensure-dir "/dev/rdsk" owner="root" group="root" mode="755"
    ensure-dir "/dev/usb" owner="root" group="root" mode="755"

    // Populate device nodes
    devfsadm

    // System configuration
    file destination="/etc/inet/hosts" source="files/etc/hosts"
    file destination="/etc/nodename" source="files/etc/nodename"
    ensure-symlink "/etc/nsswitch.conf" target="/etc/nsswitch.dns"

    // Root password
    shadow username="root" password="$5$rounds=10000$..."
}
```
