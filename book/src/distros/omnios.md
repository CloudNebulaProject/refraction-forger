# OmniOS

OmniOS is Forger's primary target distribution. It's a server-focused illumos distribution maintained by the OmniOS Community Edition (OmniOSce) project.

## Release Branches

| Branch | URL Path | Use Case |
|---|---|---|
| **bloody** | `/bloody/core/` | Rolling development, latest packages |
| **stable** (e.g., r151050) | `/r151050/core/` | Production, LTS releases |

### Choosing a Branch

- Use **bloody** for CI images, development, and testing the latest software
- Use **stable** for production deployments where predictability matters

## Minimal Spec

```kdl
metadata name="omnios-base" version="1.0.0"

repositories {
    publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
    publisher name="extra.omnios" origin="https://pkg.omnios.org/bloody/extra/"
}

incorporation "entire"

certificates {
    ca publisher="omnios" certfile="omniosce-ca.cert.pem"
}

variants {
    set name="opensolaris.zone" value="global"
}

packages {
    package "/editor/vim"
    package "/network/openssh-server"
}
```

### Required Elements

- **`incorporation "entire"`**: Pins all packages to a consistent version set. Without this, you may get incompatible package versions.
- **`certificates`**: OmniOS packages are signed. The CA certificate file (`omniosce-ca.cert.pem`) must be available relative to the spec.
- **`variants` with `opensolaris.zone=global`**: Required for bootable VM images. Omitting this may exclude kernel modules.

## Common Packages

### Core System

```kdl
packages {
    package "/editor/vim"
    package "/network/openssh-server"
    package "/network/rsync"
    package "/service/network/ntpsec"
    package "/web/curl"
    package "/web/wget"
}
```

### Cloud & Virtualization

```kdl
packages {
    package "/system/cloud-init"
    package "/driver/virtio/vioif"      // Virtio network
    package "/driver/virtio/vioblk"     // Virtio block storage
    package "/driver/virtio/vio9p"      // 9p filesystem sharing
    package "/driver/virtio/vioscsi"    // Virtio SCSI
    package "/driver/virtio/viorand"    // Virtio RNG
}
```

### Development

```kdl
packages if="build" {
    package "/developer/build-essential"
    package "/ooce/developer/omnios-build-tools"
    package "/developer/build/gnu-make"
}
```

### Rust Toolchain

```kdl
packages if="rust" {
    package "/ooce/developer/rust"
    package "/developer/versioning/git"
}
```

## Boot Configuration

OmniOS VM images need console and boot configuration through overlays:

### Serial Console (115200 baud)

```kdl
overlays {
    file destination="/boot/conf.d/console" source="files/boot_console.115200"
    file destination="/etc/ttydefs" source="files/ttydefs.115200"
    file destination="/etc/default/init" source="files/default_init.utc"
}
```

The boot console file configures which console device is used:
- **`ttya`**: Serial console (standard for cloud VMs)
- **`text`**: Framebuffer (for interactive debugging)

### SMF Profiles

```kdl
overlays {
    ensure-symlink "/etc/svc/profile/generic.xml" target="generic_limited_net.xml"
    ensure-symlink "/etc/svc/profile/inetd_services.xml" target="inetd_generic.xml"
    ensure-symlink "/etc/svc/profile/platform.xml" target="platform_none.xml"
    ensure-symlink "/etc/svc/profile/name_service.xml" target="ns_dns.xml"
}
```

## QCOW2 Target Settings

For OmniOS, always use ZFS with UEFI:

```kdl
target "vm" kind="qcow2" {
    disk-size "2G"
    bootloader "uefi"
    filesystem "zfs"
    pool {
        property name="ashift" value="12"
    }
}
```

- **`ashift=12`**: 4K sector alignment, correct for modern disks and virtual storage
- **UEFI**: Standard boot method for OmniOS

## Complete Example

See `images/omnios-bloody-disk.kdl` in the repository for a full bootable OmniOS image spec, or the [Example Specs](../reference/examples.md) chapter.

## OmniOS Extra Publisher

The `extra.omnios` publisher provides community-maintained packages not in the core repository, including:

- Rust (`/ooce/developer/rust`)
- Go (`/ooce/developer/go`)
- Python versions (`/ooce/runtime/python-*`)
- Node.js (`/ooce/runtime/node-*`)
- Build tools (`/ooce/developer/omnios-build-tools`)

Always add this publisher if you need development tools or language runtimes.
