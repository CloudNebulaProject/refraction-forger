# Crate Structure

Forger is a Cargo workspace with five crates, each with a clear responsibility.

## Workspace Layout

```
crates/
├── forger/          CLI entry point
├── spec-parser/     KDL parsing and resolution
├── forge-engine/    Build execution (Phase 1 + Phase 2)
├── forge-builder/   Remote VM builds
└── forge-oci/       OCI image and registry operations
```

## forger (CLI)

The binary crate. Defines five subcommands via clap:

| Command | Description |
|---|---|
| `build` | Build image from spec |
| `validate` | Parse and check a spec |
| `inspect` | Show resolved, profile-filtered spec |
| `push` | Push artifact to OCI registry |
| `targets` | List targets in a spec |

This crate is thin — it parses CLI arguments and delegates to the library crates.

## spec-parser

Handles everything related to KDL spec files:

- **Parsing**: Uses the `knuffel` crate to deserialize KDL into Rust structs (`ImageSpec`, `Target`, `Package`, etc.)
- **Resolution**: Recursively resolves `base` and `include` references, merging specs while detecting circular dependencies
- **Profile filtering**: Removes conditional blocks that don't match active profiles

Key types:
- `ImageSpec` — Root spec structure
- `DistroFamily` — Enum discriminating OmniOS vs Ubuntu
- `Target` / `TargetKind` — Output target definitions
- `Overlays` — File operations (file, ensure-dir, ensure-symlink, shadow, devfsadm, remove-files)

## forge-engine

The build engine implementing both phases:

- **Phase 1** (`phase1/mod.rs`): Distro-specific rootfs assembly
  - IPS operations for OmniOS
  - debootstrap + APT for Ubuntu
  - Overlay application (shared across distros)
- **Phase 2** (`phase2/mod.rs`): Target production
  - `qcow2_zfs.rs`: ZFS pool creation, BE management
  - `qcow2_ext4.rs`: Partitioning, ext4 formatting
  - OCI image layout creation
  - Tar artifact packaging

Key abstraction: **`ToolRunner` trait** — wraps all external tool execution (`pkg`, `apt`, `qemu-img`, `zfs`, `parted`, etc.) through a single interface. The real implementation (`SystemToolRunner`) uses `tokio::process::Command`. This trait enables testing without root access.

## forge-builder

Manages remote builds via ephemeral VMs:

1. Resolve builder image (OCI ref, URL, or path)
2. Generate ephemeral SSH keypair
3. Create cloud-init config
4. Start QEMU via `vm-manager` crate (user-mode networking)
5. SSH connect with retry (up to 120s)
6. Transfer forger binary + spec + files via SCP
7. Execute remote build via SSH
8. Download artifacts
9. Destroy VM (guaranteed cleanup)

Uses the external `vm-manager` crate for hypervisor abstraction (`RouterHypervisor` auto-detects QEMU/bhyve).

## forge-oci

OCI-specific operations:

- **tar_layer**: Compress rootfs into gzip layer
- **manifest**: Build OCI config and manifest JSON
- **layout**: Write OCI Image Layout directory structure
- **artifact**: Package QCOW2 as OCI artifact with custom media types
- **registry**: Push to OCI registries (token auth, basic auth, anonymous)
- **AuthConfig**: GHCR token auto-detection, auth file parsing

## Dependency Flow

```
forger
  ├── spec-parser
  ├── forge-engine
  │     └── spec-parser
  ├── forge-builder
  │     ├── spec-parser
  │     └── vm-manager (external)
  └── forge-oci
```

All crates are async (tokio) and use miette for error diagnostics.

## Key External Dependencies

| Category | Crate | Purpose |
|---|---|---|
| KDL parsing | `knuffel` 3.2 | Spec deserialization |
| CLI | `clap` 4.5 | Argument parsing |
| Async | `tokio` 1 | Runtime, process, fs |
| OCI | `oci-spec`, `oci-client` | Image spec, registry client |
| SSH | `ssh2` | Remote builder communication |
| Errors | `miette`, `thiserror` | Rich diagnostics |
| VM | `vm-manager` (path dep) | Hypervisor abstraction |
