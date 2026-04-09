# OCI Registry Push

Forger can push built images directly to OCI-compliant registries, including GitHub Container Registry (GHCR), Docker Hub, and self-hosted registries.

## Auto-Push (Target Configuration)

Set `push-to` on a target to automatically push after a successful build:

```kdl
target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    push-to "ghcr.io/myorg/omnios-image:latest"
}

target "container" kind="oci" {
    push-to "ghcr.io/myorg/omnios-container:latest"
}
```

Skip the push with:

```bash
forger build --spec my-image.kdl --skip-push
```

## Manual Push

Push a previously built image:

```bash
# Push OCI Image Layout
forger push --image output/container/ --reference ghcr.io/myorg/myimage:latest

# Push QCOW2 as OCI artifact
forger push --image output/vm.qcow2 --reference ghcr.io/myorg/myvm:latest --artifact
```

### Options

| Flag | Description |
|---|---|
| `--image <PATH>` | Path to OCI Image Layout directory or QCOW2 file |
| `--reference <REF>` | Registry reference (e.g., `ghcr.io/org/image:tag`) |
| `--artifact` | Push QCOW2 as OCI artifact (custom media types) |
| `--auth-file <PATH>` | JSON auth file for registry authentication |

## Authentication

### GitHub Container Registry (GHCR)

Forger automatically uses the `GITHUB_TOKEN` environment variable when pushing to `ghcr.io`:

```bash
export GITHUB_TOKEN=ghp_...
forger build --spec my-image.kdl
```

In GitHub Actions, the token is available automatically:

```yaml
- name: Build and push
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  run: forger build --spec images/omnios-rust-ci.kdl
```

### Auth File

For other registries, provide a JSON auth file:

```json
{
    "username": "myuser",
    "password": "mypassword"
}
```

Or with a token:

```json
{
    "token": "my-registry-token"
}
```

```bash
forger push --image output/container/ \
  --reference registry.example.com/myimage:latest \
  --auth-file auth.json
```

### Anonymous Push

Local registries (localhost, 127.0.0.1) are accessed without authentication over HTTP (insecure mode).

## QCOW2 as OCI Artifact

When pushing QCOW2 images with `--artifact`, Forger uses custom OCI media types:

- Config: `application/vnd.cloudnebula.qcow2.config.v1+json`
- Layer: `application/vnd.cloudnebula.qcow2.layer.v1`

This allows distributing VM disk images through container registries alongside container images, using a unified registry infrastructure.

## Pulling QCOW2 Artifacts

QCOW2 artifacts pushed to a registry can be pulled back as builder images or for deployment:

```kdl
builder {
    image "oci://ghcr.io/myorg/omnios-builder:latest"
}
```

Forger resolves `oci://` references by pulling from the registry.
