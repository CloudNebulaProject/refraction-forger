# From omnios-image-builder

This guide helps you migrate from the shell-based `omnios-image-builder` to Forger.

## Architecture Comparison

| omnios-image-builder | Forger |
|---|---|
| Shell scripts (`strap.sh`, `aws.sh`, etc.) | Single `forger` binary |
| JSON templates with step arrays | KDL specs with declarative blocks |
| ZFS snapshots for caching | `base` directive for caching |
| Requires illumos host with ZFS + pfexec | Any host (remote builder VM) |
| Manual pipeline (01-strap → 02-image → 03-archive → final) | Implicit pipeline via `base`/`include` |

## Mapping Concepts

### JSON Steps → KDL Blocks

The old JSON templates used a `steps` array with typed objects:

```json
{
  "steps": [
    { "t": "pkg_image_create", "publisher": "omnios", "uri": "https://..." },
    { "t": "pkg_install", "pkgs": ["entire"] },
    { "t": "pkg_change_variant", "variant": "opensolaris.zone", "value": "global" }
  ]
}
```

In Forger, these become declarative blocks:

```kdl
repositories {
    publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
}

incorporation "entire"

variants {
    set name="opensolaris.zone" value="global"
}

packages {
    package "/editor/vim"
}
```

### Step Type Mapping

| JSON Step | KDL Equivalent |
|---|---|
| `pkg_image_create` | `repositories { publisher ... }` (implicit) |
| `pkg_set_publisher` | `repositories { publisher ... }` |
| `pkg_install` | `packages { package "..." }` |
| `pkg_change_variant` | `variants { set ... }` |
| `pkg_approve_ca_cert` | `certificates { ca ... }` |
| `pkg_uninstall` | Not needed (don't install unwanted packages) |
| `pkg_purge_history` | Handled automatically |
| `pkg_set_property` | Handled automatically |
| `ensure_file` | `overlays { file ... }` |
| `ensure_dir` | `overlays { ensure-dir ... }` |
| `ensure_symlink` | `overlays { ensure-symlink ... }` |
| `remove_files` | `overlays { remove-files ... }` |
| `shadow` | `overlays { shadow ... }` |
| `devfsadm` | `overlays { devfsadm }` |
| `seed_smf` | Handled automatically |
| `include` | `include "file.kdl"` |
| `create_be` | Handled automatically (QCOW2 target) |
| `unpack_tar` | Handled automatically (`base` directive) |
| `make_bootable` | `target ... { bootloader "uefi" }` |
| `pack_tar` | `target "..." kind="artifact"` |

### Three-Stage Pipeline → Base + Child

**Old approach** (three JSON files + shell orchestration):

```
01-strap.json  → pkg install entire → snapshot "strap"
02-image.json  → pkg install extras → snapshot "image"
03-archive.json → pack_tar → omnios-bloody.tar.gz
aws.json       → unpack_tar + make_bootable → raw disk
```

**New approach** (two KDL files):

```kdl
// omnios-base.kdl
repositories { publisher name="omnios" ... }
incorporation "entire"
packages { package "/editor/vim" ... }
```

```kdl
// omnios-disk.kdl
base "omnios-base.kdl"
include "devfs.kdl"
include "common.kdl"

packages { package "/system/cloud-init" }
overlays { file destination="/boot/conf.d/console" ... }

target "vm" kind="qcow2" { ... }
```

The `base` directive replaces the ZFS snapshot pipeline. The parent's output is cached, and the child builds on top.

### Pool Configuration

**Old** (`pool` in JSON):

```json
{
  "pool": { "name": "rpool", "ashift": 12, "uefi": true, "size": 2000 }
}
```

**New** (in target block):

```kdl
target "vm" kind="qcow2" {
    disk-size "2000M"
    bootloader "uefi"
    filesystem "zfs"
    pool {
        property name="ashift" value="12"
    }
}
```

### Output Formats

**Old**: Raw disk images only (convert to QCOW2/AMI externally)

**New**: QCOW2 natively, plus OCI container images and tar artifacts

### Cloud Provider Scripts

The old `aws.sh`, `digitalocean.sh`, and `smartosbhyve.sh` scripts are replaced by profiles or separate specs:

```kdl
// Cloud-provider-specific overlays via profiles
overlays if="aws" {
    file destination="/boot/conf.d/console" source="files/boot_console.aws"
}

overlays if="digitalocean" {
    file destination="/boot/conf.d/console" source="files/boot_console.do"
}
```

## Migration Checklist

1. Identify your JSON templates and their step sequences
2. Map each step to the equivalent KDL block (see table above)
3. Split the three-stage pipeline into base + child specs
4. Move overlay files to a `files/` directory relative to the specs
5. Replace `setup.sh` dependency management with `forger` binary installation
6. Test with `forger validate` and `forger inspect` before building
