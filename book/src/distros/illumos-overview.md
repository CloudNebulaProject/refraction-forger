# illumos Overview

Forger's primary focus is the **illumos** ecosystem. This chapter provides essential background for image developers working with illumos distributions.

## What is illumos?

illumos is a Unix operating system kernel derived from OpenSolaris. It powers several distributions including OmniOS, OpenIndiana, and SmartOS. Key technologies that distinguish illumos from Linux:

### ZFS

ZFS is the default and recommended filesystem for illumos. It provides:

- **Copy-on-write**: Snapshots and clones are instant and space-efficient
- **Boot Environments (BEs)**: Multiple bootable system states on the same pool
- **Data integrity**: End-to-end checksumming
- **Built-in compression**: LZ4 by default on modern pools

Forger creates ZFS pools natively for QCOW2 targets on illumos.

### IPS (Image Packaging System)

IPS is illumos's package manager. Key concepts:

- **Publishers**: Named package repositories (e.g., `omnios`, `extra.omnios`)
- **Incorporations**: Meta-packages that constrain version compatibility (e.g., `entire`)
- **Variants**: Facets that select package subsets (e.g., `opensolaris.zone=global`)
- **FMRIs**: Hierarchical package names (e.g., `/network/openssh-server`)
- **Signed packages**: CA-verified package integrity

Forger wraps IPS operations directly — no shell scripting needed.

### SMF (Service Management Facility)

SMF is illumos's service manager (similar to systemd on Linux, but predates it). Services are defined by XML manifests and managed via profiles:

- **Profiles** control which services are enabled at boot
- **generic_limited_net.xml**: Basic networking
- **inetd_generic.xml**: Internet daemon services
- **platform_none.xml**: Platform-specific (none for VM images)
- **ns_dns.xml**: DNS name service

Forger configures SMF profiles through symlink overlays:

```kdl
overlays {
    ensure-symlink "/etc/svc/profile/generic.xml" target="generic_limited_net.xml"
    ensure-symlink "/etc/svc/profile/name_service.xml" target="ns_dns.xml"
}
```

### Zones

illumos zones are lightweight OS-level containers (similar to LXC/Docker, but predating both). The `opensolaris.zone` variant controls whether packages include global-zone-only components:

- `global`: Full system including kernel modules, boot components, and hardware drivers
- `nonglobal`: Zone-only packages (no kernel or hardware support)

For VM images, always use `global`:

```kdl
variants {
    set name="opensolaris.zone" value="global"
}
```

### Device Filesystem (devfs)

illumos manages device nodes through `devfsadm`, which discovers hardware and creates entries in `/dev`. For image building, this means:

1. Clean up stale device nodes from the build environment
2. Run `devfsadm` to create correct entries for the target hardware
3. Ensure required directories exist (`/dev/dsk`, `/dev/rdsk`, `/dev/cfg`, `/dev/usb`)

Forger handles this through the `devfsadm` overlay and the conventional `devfs.kdl` include.

## illumos Distributions

| Distribution | Focus | Publisher URL |
|---|---|---|
| **OmniOS** | Server/cloud, minimal, stable | `pkg.omnios.org` |
| **OpenIndiana** | Desktop/general-purpose, broader package set | `pkg.openindiana.org` |
| **SmartOS** | Hypervisor/container host (Joyent) | Not IPS-based |

Forger currently supports OmniOS. OpenIndiana support follows the same IPS path.
