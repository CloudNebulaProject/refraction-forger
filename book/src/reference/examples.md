# Example Specs

Complete, production-ready examples from the `images/` directory.

## OmniOS Bloody Base

A reusable base spec for OmniOS bloody images. This is typically used as a `base` for other specs.

```kdl
metadata name="omnios-bloody-base" version="1.0.0" description="Base OmniOS bloody image"

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
    package "/network/rsync"
    package "/service/network/ntpsec"
    package "/web/curl"
    package "/web/wget"
    package "/system/cloud-init"
}

packages if="build" {
    package "/developer/build-essential"
    package "/ooce/developer/omnios-build-tools"
    package "/developer/build/gnu-make"
}
```

## OmniOS Bloody Bootable Disk

A bootable QCOW2 VM image built on top of the base. Includes device filesystem setup, network configuration, and boot console settings.

```kdl
metadata name="omnios-bloody-disk" version="1.0.0" description="OmniOS bloody bootable disk"

base "omnios-bloody-base.kdl"
include "devfs.kdl"
include "common.kdl"

packages {
    package "/system/cloud-init"
    package "/driver/virtio/viorand"
    package "/driver/virtio/vioif"
    package "/driver/virtio/vioblk"
    package "/driver/virtio/vio9p"
    package "/driver/virtio/vioscsi"
}

overlays {
    file destination="/boot/conf.d/console" source="files/boot_console.115200"
    file destination="/etc/ttydefs" source="files/ttydefs.115200"
    file destination="/etc/default/init" source="files/default_init.utc"
    shadow username="root" password="$5$rounds=10000$..."
}

target "vm" kind="qcow2" {
    disk-size "2000M"
    bootloader "uefi"
    filesystem "zfs"
    pool {
        property name="ashift" value="12"
    }
}
```

## OmniOS Rust CI Image

An OmniOS image with the Rust toolchain for continuous integration.

```kdl
metadata name="omnios-rust-ci" version="1.0.0" description="OmniOS Rust CI image"

base "omnios-bloody-base.kdl"
include "devfs.kdl"
include "common.kdl"

packages {
    package "/ooce/developer/rust"
    package "/developer/versioning/git"
    package "/system/cloud-init"
    package "/driver/virtio/viorand"
    package "/driver/virtio/vioif"
    package "/driver/virtio/vioblk"
    package "/driver/virtio/vio9p"
    package "/driver/virtio/vioscsi"
}

builder {
    image "oci://ghcr.io/cloudnebulaproject/omnios-builder:latest"
    vcpus 4
    memory 4096
    disk 20
}

target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    filesystem "zfs"
    pool {
        property name="ashift" value="12"
    }
    push-to "ghcr.io/cloudnebulaproject/omnios-rust:latest"
}
```

## Ubuntu Rust CI Image

An Ubuntu 22.04 image with the Rust toolchain.

```kdl
metadata name="ubuntu-rust-ci" version="1.0.0" description="Ubuntu 22.04 Rust CI image"

distro "ubuntu-22.04"

repositories {
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy" components="main universe"
}

packages {
    package "build-essential"
    package "rustc"
    package "cargo"
    package "git"
    package "curl"
    package "openssh-server"
    package "cloud-init"
    package "linux-image-generic"
    package "grub-efi-amd64-bin"
    package "libssl-dev"
    package "pkg-config"
}

customization {
    user "ci"
}

builder {
    image "oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest"
    vcpus 4
    memory 4096
    disk 20
}

target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "grub-efi-amd64-bin"
    filesystem "ext4"
    push-to "ghcr.io/cloudnebulaproject/ubuntu-rust:latest"
}
```

## Device Filesystem Include

Standard device node setup for bootable illumos images.

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

## Common System Configuration Include

Standard network and SMF profile setup for illumos images.

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
