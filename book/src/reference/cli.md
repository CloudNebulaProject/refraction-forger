# CLI Reference

## forger build

Build an image from a spec file.

```
forger build [OPTIONS] --spec <PATH>
```

| Option | Description |
|---|---|
| `--spec <PATH>` | Path to the KDL spec file (required) |
| `--target <NAME>` | Build only this target (default: all) |
| `--profile <PROFILE>` | Activate a profile (repeatable) |
| `--output-dir <PATH>` | Output directory (default: `./output`) |
| `--local` | Force local build (skip builder VM) |
| `--use-builder` | Force build inside a builder VM |
| `--skip-push` | Don't push to registry after build |
| `--builder-image <SPEC>` | Override builder VM image |

### Examples

```bash
# Build all targets
forger build --spec images/omnios-bloody-disk.kdl

# Build specific target with profile
forger build --spec images/omnios-bloody-disk.kdl --target vm --profile build

# Force local build, custom output directory
forger build --spec images/omnios-bloody-disk.kdl --local --output-dir /tmp/images

# Build with custom builder image
forger build --spec images/ubuntu-rust-ci.kdl --builder-image /path/to/builder.qcow2
```

## forger validate

Parse and check a spec file for errors.

```
forger validate --spec <PATH>
```

| Option | Description |
|---|---|
| `--spec <PATH>` | Path to the KDL spec file (required) |

Checks:
- KDL syntax
- Schema conformance
- Include/base resolution
- Circular dependency detection

### Example

```bash
forger validate --spec images/omnios-bloody-disk.kdl
```

## forger inspect

Parse, resolve, and apply profiles, then display the resolved spec.

```
forger inspect [OPTIONS] --spec <PATH>
```

| Option | Description |
|---|---|
| `--spec <PATH>` | Path to the KDL spec file (required) |
| `--profile <PROFILE>` | Activate a profile (repeatable) |

### Examples

```bash
# Inspect without profiles
forger inspect --spec images/omnios-bloody-disk.kdl

# Inspect with build profile to see what's included
forger inspect --spec images/omnios-bloody-disk.kdl --profile build
```

## forger push

Push a built artifact to an OCI registry.

```
forger push [OPTIONS] --image <PATH> --reference <REF>
```

| Option | Description |
|---|---|
| `--image <PATH>` | Path to OCI Image Layout directory or QCOW2 file |
| `--reference <REF>` | Registry reference (e.g., `ghcr.io/org/image:tag`) |
| `--auth-file <PATH>` | JSON auth file (optional) |
| `--artifact` | Push QCOW2 as OCI artifact |

### Examples

```bash
# Push OCI image
forger push --image output/container/ --reference ghcr.io/myorg/myimage:latest

# Push QCOW2 as artifact
forger push --image output/vm.qcow2 --reference ghcr.io/myorg/myvm:latest --artifact

# Push with auth file
forger push --image output/container/ \
  --reference registry.example.com/myimage:latest \
  --auth-file auth.json
```

## forger targets

List targets defined in a spec file.

```
forger targets --spec <PATH>
```

| Option | Description |
|---|---|
| `--spec <PATH>` | Path to the KDL spec file (required) |

### Example

```bash
forger targets --spec images/omnios-bloody-disk.kdl
```

Output:

```
vm (qcow2)
```

## Environment Variables

| Variable | Description |
|---|---|
| `GITHUB_TOKEN` | Used for GHCR authentication when pushing |
| `RUST_LOG` | Logging level (e.g., `info`, `debug`, `trace`) |
