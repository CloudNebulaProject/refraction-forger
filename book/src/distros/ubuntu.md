# Ubuntu

Ubuntu is Forger's secondary target, providing Linux support for teams that need both illumos and Linux images from the same tooling.

## How Ubuntu Builds Differ

| Aspect | OmniOS | Ubuntu |
|---|---|---|
| Bootstrap | `pkg image-create` | `debootstrap` |
| Package manager | IPS (`pkg`) | APT (`apt`) |
| Repository config | Publishers | `sources.list` |
| Default filesystem | ZFS | ext4 |
| Bootloader | UEFI (illumos) | `grub-efi-amd64-bin` |
| Init system | SMF | systemd |

## Minimal Spec

```kdl
metadata name="ubuntu-base" version="1.0.0" description="Ubuntu 22.04 base"

distro "ubuntu-22.04"

repositories {
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy" components="main universe"
}

packages {
    package "openssh-server"
    package "curl"
}

target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "grub-efi-amd64-bin"
    filesystem "ext4"
}
```

### Key Differences from OmniOS

- **`distro "ubuntu-22.04"`** is required — without it, Forger assumes OmniOS
- **No incorporation, variants, or certificates** — these are IPS concepts
- **Bootloader is `grub-efi-amd64-bin`** — the Ubuntu GRUB EFI package
- **Filesystem is `ext4`** — ZFS is possible but not the Ubuntu default

## Repositories

Ubuntu uses APT mirrors with suite and component selection:

```kdl
repositories {
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy" components="main universe"
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy-updates" components="main universe"
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy-security" components="main universe"
}
```

### Components

- **main**: Officially supported free software
- **universe**: Community-maintained free software
- **restricted**: Proprietary drivers
- **multiverse**: Non-free software

For most server images, `main universe` covers all needed packages.

## Common Packages

### Core System

```kdl
packages {
    package "openssh-server"
    package "curl"
    package "git"
    package "cloud-init"
    package "linux-image-generic"
}
```

> **Important**: Include `linux-image-generic` for bootable VM images. Without a kernel, the image won't boot.

### Build Tools

```kdl
packages if="build" {
    package "build-essential"
    package "pkg-config"
    package "libssl-dev"
}
```

### Rust Toolchain

```kdl
packages if="rust" {
    package "rustc"
    package "cargo"
    package "build-essential"
    package "libssl-dev"
    package "pkg-config"
}
```

## Builder VM

Ubuntu builds need an Ubuntu builder VM. Specify it explicitly or let Forger use the default:

```kdl
builder {
    image "oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest"
    vcpus 4
    memory 4096
    disk 20
}
```

## Complete Example

```kdl
metadata name="ubuntu-rust-ci" version="1.0.0" description="Ubuntu Rust CI image"

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

## Future: IPS on Linux

A long-term goal of the Forger project is to bring IPS to Linux via a Rust implementation. This would allow Linux images to use the same publisher-based, signed-package, incorporation-constrained model that makes illumos packaging robust. When this is available, Ubuntu and other Linux distros will gain IPS as an alternative package manager option within Forger.
