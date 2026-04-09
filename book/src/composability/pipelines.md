# Multi-Stage Pipelines

Forger's composability model enables multi-stage build pipelines where each stage's output feeds as input to the next. This is the key to fast, cacheable image builds.

## The Pipeline Pattern

A typical pipeline for a bootable OmniOS VM image looks like this:

```
Stage 1: Base (expensive, cached)
  omnios-base.kdl
    → Initializes IPS
    → Sets publishers
    → Installs core packages
    → Produces: artifact (tar archive)

Stage 2: Image (fast, incremental)
  omnios-disk.kdl (base: omnios-base.kdl)
    → Consumes base artifact
    → Adds cloud-init, drivers
    → Applies overlays (config files, device nodes)
    → Produces: QCOW2 image
```

The first build runs both stages. Subsequent builds skip Stage 1 if the base hasn't changed, jumping straight to Stage 2.

## Designing Your Pipeline

### Identify the Cache Boundary

The most expensive operation in image building is package installation — downloading and extracting hundreds of packages from a repository. Put this in the base spec:

```kdl
// omnios-base.kdl — the slow part, cached
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
    package "/network/rsync"
    package "/service/network/ntpsec"
    package "/web/curl"
    package "/web/wget"
}
```

### Derive Specific Images

Then create derivative specs that add target-specific configuration:

```kdl
// omnios-disk.kdl — fast, builds on cached base
base "omnios-base.kdl"
include "devfs.kdl"
include "common.kdl"

packages {
    package "/system/cloud-init"
    package "/driver/virtio/vioif"
    package "/driver/virtio/vioblk"
}

overlays {
    file destination="/boot/conf.d/console" source="files/boot_console.115200"
    shadow username="root" password="$5$rounds=10000$..."
}

target "vm" kind="qcow2" {
    disk-size "2G"
    bootloader "uefi"
    filesystem "zfs"
    pool {
        property name="ashift" value="12"
    }
}
```

### Multiple Derivatives from One Base

```
omnios-base.kdl
  ├── omnios-disk.kdl          → QCOW2 VM image
  ├── omnios-rust-ci.kdl       → Rust CI image (adds rust, git, build tools)
  ├── omnios-container.kdl     → OCI container
  └── omnios-aws.kdl           → AWS-specific VM image
```

Each derivative shares the same cached base, so building all four images only runs the expensive Stage 1 once.

## Comparison with Predecessor Tools

### omnios-image-builder (Shell + JSON)

The old `omnios-image-builder` achieved the same pattern with ZFS snapshots:

```
01-strap.json  → pkg install entire → ZFS snapshot "strap"
02-image.json  → pkg install extras → ZFS snapshot "image"
03-archive.json → pack_tar → omnios-bloody.tar.gz
aws.json       → unpack_tar + make_bootable → raw disk
```

Each ZFS snapshot was a cache point. The `-f` flag forced a full rebuild.

Forger replaces this with the `base` directive. No ZFS on the build host required. No manual snapshot management. The caching is implicit in the spec relationship.

### Packer (HCL)

Packer has no native multi-stage caching. Each build starts from an ISO installation, runs provisioner scripts, and captures the result. There's no way to say "skip the OS install and start from here."

Forger's pipeline model is fundamentally more efficient for iterative development.
