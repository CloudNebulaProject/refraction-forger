# Build Modes

Forger supports two build modes: **local** and **remote**. The mode is selected automatically based on your environment, or you can force it with CLI flags.

## Local Build

A local build runs directly on your host machine. This is the fastest option but requires:

- The target distro's package manager to be available
- Root/sudo access for filesystem operations
- Matching architecture (e.g., building OmniOS images on OmniOS)

```bash
forger build --spec my-image.kdl --local
```

Use `--local` to skip builder VM detection and force a local build.

## Remote Build (Builder VM)

When your host doesn't match the target OS — for example, building OmniOS images from a Linux workstation — Forger spins up an ephemeral builder VM:

1. Downloads or uses a cached builder image (OCI reference, URL, or local file)
2. Creates a cloud-init configuration with an ephemeral SSH keypair
3. Starts a QEMU VM with user-mode networking (no root needed on host)
4. Transfers the `forger` binary, spec file, and overlay files via SSH
5. Runs the build inside the VM
6. Downloads the finished artifacts
7. Destroys the VM

```bash
forger build --spec my-image.kdl --use-builder
```

Use `--use-builder` to force a remote build even when local build is possible.

### Default Builder Images

If no builder is specified in the spec or on the CLI, Forger uses sensible defaults:

| Target Distro | Default Builder Image |
|---|---|
| OmniOS | `oci://ghcr.io/cloudnebulaproject/omnios-builder:latest` |
| Ubuntu | `oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest` |

### Override the Builder Image

From the CLI:

```bash
forger build --spec my-image.kdl --builder-image oci://my-registry/my-builder:v1
```

Or in the spec file (see [Builder Configuration](../spec/builder.md)).

## How Forger Chooses

When neither `--local` nor `--use-builder` is specified, Forger checks whether the current host can satisfy the build requirements (package manager availability, OS match). If it can, it builds locally. Otherwise, it falls back to a builder VM.
