# OCI Container Images

OCI targets produce container images compatible with Docker, Podman, and any OCI-compliant runtime.

## Syntax

```kdl
target "container" kind="oci" {
    entrypoint command="/bin/sh"
    environment {
        set "PATH" "/usr/bin:/bin:/usr/sbin:/sbin"
        set "TZ" "UTC"
    }
}
```

## How OCI Targets Work

Phase 2 for an OCI target:

1. **Compress** the Phase 1 rootfs into a gzip tar layer
2. **Compute** SHA256 digests for all blobs
3. **Build** OCI config JSON (entrypoint, environment, layer diff IDs)
4. **Build** OCI manifest JSON (media types, blob references, sizes)
5. **Write** an OCI Image Layout directory:

```
output/container/
├── oci-layout          # {"imageLayoutVersion": "1.0.0"}
├── index.json          # Points to manifest
└── blobs/
    └── sha256/
        ├── <manifest>  # OCI manifest JSON
        ├── <config>    # OCI config JSON
        └── <layer>     # Compressed rootfs layer
```

## Configuration

### Entrypoint

The command to run when the container starts:

```kdl
entrypoint command="/usr/sbin/sshd"
```

If omitted, no entrypoint is set and the container runtime's default applies.

### Environment Variables

```kdl
environment {
    set "PATH" "/usr/bin:/bin:/usr/sbin:/sbin"
    set "LANG" "C.UTF-8"
    set "TZ" "UTC"
}
```

## Using the Output

### Load into Docker

```bash
# From OCI Image Layout directory
docker load < output/container/

# Or use skopeo
skopeo copy oci:output/container docker-daemon:myimage:latest
```

### Load into Podman

```bash
podman load < output/container/
```

### Push to Registry

Use the `push` command or `push-to` in the target:

```bash
forger push --image output/container/ --reference ghcr.io/myorg/myimage:latest
```

Or configure auto-push in the spec (see [OCI Registry Push](./registry.md)).

## OmniOS Containers

OmniOS in a container is useful for CI builds and lightweight services:

```kdl
metadata name="omnios-container" version="1.0.0"

repositories {
    publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
}

incorporation "entire"

variants {
    set name="opensolaris.zone" value="nonglobal"
}

packages {
    package "/editor/vim"
    package "/web/curl"
}

target "container" kind="oci" {
    entrypoint command="/bin/bash"
    environment {
        set "PATH" "/usr/bin:/bin:/usr/sbin:/sbin"
    }
}
```

Note: For containers, use `opensolaris.zone=nonglobal` to exclude kernel modules and hardware drivers that aren't needed in a container context.
