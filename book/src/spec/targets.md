# Targets

Targets define what Forger produces from the assembled rootfs. Each spec can have multiple targets, and you can build them selectively.

## Target Kinds

| Kind | Description | Typical Use |
|---|---|---|
| `qcow2` | Bootable VM disk image | Cloud VMs, hypervisors |
| `oci` | OCI container image | Container runtimes |
| `artifact` | Tar archive of the rootfs | Intermediate build stage, embedding |

## QCOW2 Target

Produces a bootable virtual machine disk image:

```kdl
target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    filesystem "zfs"
    pool {
        property name="ashift" value="12"
    }
    push-to "ghcr.io/myorg/my-image:latest"
}
```

| Property | Required | Default | Description |
|---|---|---|---|
| `disk-size` | Yes | — | Disk size (e.g., `"2G"`, `"2000M"`) |
| `bootloader` | Yes | — | `"uefi"`, `"grub"`, or `"grub-efi-amd64-bin"` |
| `filesystem` | No | Distro default | `"zfs"` or `"ext4"` |
| `push-to` | No | — | OCI registry reference for auto-push |
| `pool` | No | — | ZFS pool properties (ZFS only) |

### ZFS Pool Properties

```kdl
pool {
    property name="ashift" value="12"
}
```

`ashift=12` sets the ZFS block alignment to 4K sectors, which is correct for modern storage.

### Bootloader Options

- **`uefi`**: UEFI bootloader — recommended for illumos and modern Linux
- **`grub`**: Legacy GRUB — for BIOS-boot OmniOS images
- **`grub-efi-amd64-bin`**: GRUB EFI for Ubuntu — use with ext4

## OCI Target

Produces an OCI container image:

```kdl
target "container" kind="oci" {
    entrypoint command="/bin/sh"
    environment {
        set "PATH" "/usr/bin:/bin:/usr/sbin:/sbin"
        set "TZ" "UTC"
    }
}
```

| Property | Required | Description |
|---|---|---|
| `entrypoint` | No | Container entrypoint command |
| `environment` | No | Environment variables |

The OCI target produces an OCI Image Layout directory containing:
- Compressed tar.gz layer(s) of the rootfs
- OCI manifest and config JSON
- SHA256-based blob digests

## Artifact Target

Produces a tar archive of the rootfs:

```kdl
target "archive" kind="artifact" {
}
```

Artifact targets are the primary mechanism for [multi-stage pipelines](../composability/pipelines.md). A parent spec can produce an artifact that a child spec consumes as its base.

## Multiple Targets

A single spec can define multiple targets:

```kdl
target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    filesystem "zfs"
}

target "container" kind="oci" {
    entrypoint command="/usr/sbin/sshd" 
}

target "archive" kind="artifact" {
}
```

Build all targets:

```bash
forger build --spec my-image.kdl
```

Build a specific target:

```bash
forger build --spec my-image.kdl --target vm
```

## Auto-Push

When `push-to` is set on a target, Forger automatically pushes the artifact to the OCI registry after a successful build (unless `--skip-push` is used):

```bash
# Build and push
forger build --spec my-image.kdl

# Build without pushing
forger build --spec my-image.kdl --skip-push
```

## Listing Targets

View all targets in a spec:

```bash
forger targets --spec my-image.kdl
```
