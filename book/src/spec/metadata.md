# Metadata & Distro

## Metadata

The `metadata` node identifies your image:

```kdl
metadata name="omnios-base" version="1.0.0" description="Base OmniOS bloody image"
```

| Property | Required | Description |
|---|---|---|
| `name` | Yes | Image name (used in output filenames) |
| `version` | Yes | Semantic version |
| `description` | No | Human-readable description |

## Distro Selection

The `distro` node tells Forger which OS family to target. This determines the package manager, filesystem defaults, and build path.

```kdl
distro "ubuntu-22.04"
```

### Supported Values

| Distro String | Family | Package Manager | Default Filesystem |
|---|---|---|---|
| *(omitted)* | OmniOS | IPS (`pkg`) | ZFS |
| `"ubuntu-22.04"` | Ubuntu | APT (`debootstrap` + `apt`) | ext4 |

### How Distro Affects the Build

The distro string maps to a `DistroFamily` internally:

- **OmniOS**: Uses `pkg image-create` to initialize IPS, sets publishers, handles incorporations, variants, and CA certificates. ZFS is the default filesystem for QCOW2 targets.
- **Ubuntu**: Uses `debootstrap` for initial rootfs, writes `sources.list`, runs `apt update` and `apt install`. ext4 is the default filesystem for QCOW2 targets.

If no `distro` is specified, OmniOS is assumed. This reflects Forger's primary focus on the illumos ecosystem.

### Distro and Builder Images

The distro also determines the default builder VM image when no explicit builder is configured:

| Distro Family | Default Builder |
|---|---|
| OmniOS | `oci://ghcr.io/cloudnebulaproject/omnios-builder:latest` |
| Ubuntu | `oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest` |
