# Tar Artifacts

Artifact targets produce a tar archive of the assembled rootfs. This is the simplest output format and serves as the building block for multi-stage pipelines.

## Syntax

```kdl
target "archive" kind="artifact" {
}
```

## Use Cases

### Build Stage Caching

The primary use of artifacts is as intermediate outputs in a [multi-stage pipeline](../composability/pipelines.md). A parent spec produces an artifact that a child spec consumes:

```kdl
// base.kdl — produces artifact
target "base-archive" kind="artifact" {
}
```

```kdl
// disk.kdl — consumes artifact from base
base "base.kdl"

target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
}
```

### External Processing

Artifacts can be consumed by external tools for further processing:

- Import into a zone or container manually
- Feed into another build system
- Archive for distribution

### Rootfs Inspection

Build an artifact to inspect what the rootfs looks like without committing to a disk image:

```bash
forger build --spec my-image.kdl --target archive
tar -tzf output/archive.tar.gz | head -50
```

## Output

The artifact is written to the output directory as a tar archive (typically gzip-compressed). The archive contains the full rootfs tree starting from `/`.
