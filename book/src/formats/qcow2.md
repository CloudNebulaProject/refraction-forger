# QCOW2 VM Images

QCOW2 (QEMU Copy-On-Write v2) is the primary output format for bootable virtual machine images.

## How QCOW2 Targets Work

Phase 2 for a QCOW2 target follows this sequence:

1. **Create a raw disk** of the specified size
2. **Attach via loopback device** (or equivalent)
3. **Create filesystem**:
   - **ZFS**: Create pool → create boot environment dataset → mount
   - **ext4**: Partition disk → format → mount
4. **Populate** from Phase 1 rootfs (copy files into mounted filesystem)
5. **Install bootloader** (UEFI or GRUB)
6. **Finalize**:
   - ZFS: Set bootfs property → unmount → export pool
   - ext4: Unmount
7. **Detach** loopback device
8. **Convert** raw disk to QCOW2 via `qemu-img convert`

## ZFS-Based Images (illumos)

ZFS is the default and recommended filesystem for OmniOS images:

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

### ZFS Pool Details

During build, Forger creates a uniquely-named ZFS pool (e.g., `forgebuild_12345`) to avoid conflicts with existing pools on the build host. After export, the pool is named `rpool` in the final image.

### Pool Properties

```kdl
pool {
    property name="ashift" value="12"
}
```

- **`ashift=12`**: 4KB sector alignment. Use this for modern storage and virtual disks.
- Additional ZFS pool properties can be set using the same syntax.

### Boot Environments

ZFS images use boot environments (BEs), a core illumos concept. The image contains a single BE that becomes the default boot target. On first boot, the system can create new BEs for upgrades, allowing rollback to previous states.

## ext4-Based Images (Linux)

ext4 is the default filesystem for Ubuntu images:

```kdl
target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "grub-efi-amd64-bin"
    filesystem "ext4"
}
```

The disk is partitioned with an EFI System Partition and a root partition formatted as ext4.

## Disk Sizing

Specify the disk size as a string with a unit suffix:

```kdl
disk-size "2G"       // 2 gigabytes
disk-size "2000M"    // 2000 megabytes
disk-size "8G"       // 8 gigabytes
```

Choose a size that accommodates your installed packages plus reasonable free space. Typical sizes:

- Minimal OmniOS: 2G
- OmniOS with development tools: 4-8G
- Ubuntu with build tools: 8G

## Bootloader Options

| Value | Platform | Description |
|---|---|---|
| `uefi` | illumos | Native UEFI boot (recommended for OmniOS) |
| `grub` | illumos | Legacy GRUB (BIOS boot) |
| `grub-efi-amd64-bin` | Ubuntu | GRUB EFI for x86_64 Linux |

## Auto-Push

QCOW2 images can be automatically pushed to an OCI registry as artifacts:

```kdl
target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    filesystem "zfs"
    push-to "ghcr.io/myorg/omnios-image:latest"
}
```

The QCOW2 file is wrapped as an OCI artifact with custom media types (`application/vnd.cloudnebula.qcow2.*`) and pushed to the registry. This allows distributing VM images through container registries.

## Deploying QCOW2 Images

QCOW2 images work with:

- **QEMU/KVM**: Direct use (`qemu-system-x86_64 -drive file=image.qcow2,format=qcow2`)
- **libvirt/virt-manager**: Import as existing disk
- **Proxmox**: Upload to storage, create VM from disk
- **Cloud platforms**: Convert to platform-specific format if needed

To convert to raw (for AWS, DigitalOcean, etc.):

```bash
qemu-img convert -f qcow2 -O raw image.qcow2 image.raw
```
