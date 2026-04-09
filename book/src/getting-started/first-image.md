# Your First Image

Let's build a minimal OmniOS VM image. Create a file called `my-image.kdl`:

```kdl
metadata name="my-first-image" version="1.0.0" description="A minimal OmniOS image"

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

## Validate First

Before building, validate the spec to catch syntax errors:

```bash
forger validate --spec my-image.kdl
```

## Inspect the Resolved Spec

See what the build will do after resolving includes and applying profiles:

```bash
forger inspect --spec my-image.kdl
```

## List Targets

Check what targets are defined:

```bash
forger targets --spec my-image.kdl
```

Output:
```
vm (qcow2)
```

## Build

```bash
forger build --spec my-image.kdl
```

This will:

1. Detect whether to build locally or spin up a builder VM
2. **Phase 1**: Create a rootfs by initializing IPS, setting publishers, and installing packages
3. **Phase 2**: Create a 2GB raw disk, set up a ZFS pool, populate it from the rootfs, install the UEFI bootloader, and convert to QCOW2

The output lands in `./output/` by default.

## Build a Specific Target

If your spec defines multiple targets, build just one:

```bash
forger build --spec my-image.kdl --target vm
```

## Next Steps

- Add [overlays](../spec/overlays.md) for custom configuration files
- Set up [base specs](../composability/base.md) for caching
- Configure a [builder VM](../spec/builder.md) for cross-platform builds
