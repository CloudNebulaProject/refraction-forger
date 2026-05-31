# Builder Configuration

The `builder` block configures the ephemeral VM used for remote builds. This is optional — Forger selects sensible defaults when omitted.

## Syntax

```kdl
builder {
    image "oci://ghcr.io/cloudnebulaproject/omnios-builder:latest"
    binary "https://artifacts.lan/forger-{triple}" sha256="9a3f…c0"
    vcpus 4
    memory 4096
    disk 20
}
```

| Property | Required | Default | Description |
|---|---|---|---|
| `image` | No | Distro-specific default | Builder VM image (OCI ref, URL, or path) |
| `binary` | No | Dev/release fallback | Source of the `forger` binary run inside the VM (see below) |
| `vcpus` | No | Host-dependent | Number of virtual CPUs |
| `memory` | No | Host-dependent | Memory in MB |
| `disk` | No | 20 (GB) | Disk overlay size in GB |

## Builder Binary

The `binary` child overrides where Forger gets the `forger` binary that drives
Phase 2 *inside* the builder VM. It takes one argument (the source) and an
optional `sha256` property:

```kdl
builder {
    binary "http://artifacts.lan/forger-{triple}" sha256="9a3f…c0"
}
```

The source may be a local `/path`, an `http(s)://` URL, or an `oci://`
reference. The `{triple}` token expands to the builder's Rust target triple
(`x86_64-unknown-linux-gnu` for Ubuntu, `x86_64-unknown-illumos` for OmniOS),
so one line works for both Linux and illumos builders. See
[Builder Binary](./builder-binary.md) for the full treatment (source kinds,
sha256 pinning, caching, OCI artifacts, and the CLI/env overrides).

## Image Sources

The `image` property accepts three formats:

### OCI Registry Reference

```kdl
builder {
    image "oci://ghcr.io/myorg/my-builder:latest"
}
```

Pulls a QCOW2 image from an OCI registry.

### URL

```kdl
builder {
    image "https://example.com/builders/omnios-builder.qcow2"
}
```

Downloads the image directly.

### Local Path

```kdl
builder {
    image "/path/to/my-builder.qcow2"
}
```

Uses a local QCOW2 file.

## Default Builder Images

When no builder image is specified (neither in the spec nor on the CLI), Forger uses:

| Distro | Default Image |
|---|---|
| OmniOS | `oci://ghcr.io/cloudnebulaproject/omnios-builder:latest` |
| Ubuntu | `oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest` |

## How the Builder VM Works

1. **Image resolution**: Downloads or locates the builder image
2. **Cloud-init**: Generates a user-data config with an ephemeral SSH keypair and a `builder` user with passwordless sudo
3. **VM creation**: Starts QEMU with user-mode networking (no host root needed)
4. **SSH connection**: Retries SSH connection for up to 120 seconds while the VM boots
5. **Transfer**: Uploads the `forger` binary, spec files, and overlay files
6. **Build**: Executes the build command inside the VM
7. **Download**: Retrieves the build artifacts
8. **Teardown**: Destroys the VM (guaranteed, even on build failure)

## CLI Override

The builder image can be overridden from the CLI, taking precedence over the spec:

```bash
forger build --spec my-image.kdl --builder-image /path/to/custom-builder.qcow2
```

## Resource Sizing

For typical builds:

- **OmniOS base image**: 2 vCPUs, 2048 MB RAM is sufficient
- **OmniOS with Rust/build tools**: 4 vCPUs, 4096 MB RAM recommended
- **Ubuntu with build-essential**: 4 vCPUs, 4096 MB RAM recommended
- **Disk**: 20 GB is sufficient for most images; increase for large package sets
