# Base Specs (Build Caching)

The `base` directive establishes a parent-child relationship between specs. This is Forger's primary caching mechanism: **a parent spec produces an artifact, and a child spec consumes it**.

## How It Works

```
omnios-base.kdl          (parent)
  → builds artifact       (tar archive cached on disk or in registry)
    → omnios-disk.kdl     (child, consumes the artifact)
      → builds QCOW2
```

The parent handles the expensive, slow operations (OS installation, base package setup). The child adds customization and produces the final image. When the parent's output is cached, rebuilding the child skips the entire base installation.

## Syntax

In the child spec:

```kdl
base "omnios-base.kdl"

// Child-specific additions
packages {
    package "/system/cloud-init"
    package "/driver/virtio/vioif"
}

target "vm" kind="qcow2" {
    disk-size "2G"
    bootloader "uefi"
    filesystem "zfs"
}
```

The parent spec (`omnios-base.kdl`) defines the foundation:

```kdl
metadata name="omnios-base" version="1.0.0" description="Base OmniOS configuration"

repositories {
    publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
    publisher name="extra.omnios" origin="https://pkg.omnios.org/bloody/extra/"
}

incorporation "entire"

certificates {
    ca publisher="omnios" certfile="omniosce-ca.cert.pem"
}

packages {
    package "/editor/vim"
    package "/network/openssh-server"
    package "/network/rsync"
}
```

## What Gets Inherited

When a child references a parent via `base`:

- **Repositories** are merged (deduplicated by name/URL)
- **Packages** from both parent and child are installed
- **Overlays** from the parent execute first, then the child's
- **Customizations** from both are applied
- **Metadata** from the child overrides the parent
- **Targets** from the child are used (parent targets are not inherited)

## Caching Pipeline

The key insight is that `base` creates a **build stage boundary**. The parent's output (typically an artifact/tar) is the cache unit:

1. First build: parent runs Phase 1 (full package installation), produces artifact
2. Subsequent builds: if the parent spec hasn't changed, skip Phase 1 and start from the cached artifact
3. The child only needs to apply its additional packages, overlays, and produce its targets

This mirrors the strap→image→archive pipeline from the old `omnios-image-builder`, but expressed declaratively.

## Multi-Level Inheritance

Base specs can themselves have bases, creating a chain:

```
distro-base.kdl
  → platform-base.kdl
    → application-image.kdl
```

Each level adds or refines what the previous level established. Circular references are detected and rejected.

## Base vs Include

| | Base | Include |
|---|---|---|
| **Relationship** | Parent → child (produces artifact for child to consume) | Sibling (shared steps imported) |
| **Caching** | Yes — parent output is the cache boundary | No — just DRY for config |
| **Merging** | Full merge with child overrides | Steps imported as-is |
| **Targets** | Child's targets used | No target interaction |
