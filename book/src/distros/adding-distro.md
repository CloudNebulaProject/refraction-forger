# Adding a New Distro

Forger's distro support is built around the `DistroFamily` abstraction. Adding a new distribution means extending this abstraction at the spec-parsing and build-engine levels.

## Architecture Overview

The distro system has three touch points:

1. **`spec-parser`**: Maps distro strings to `DistroFamily` enum
2. **`forge-engine` Phase 1**: Distro-specific rootfs assembly logic
3. **`forge-builder`**: Default builder image selection

## Step 1: Extend the DistroFamily Enum

In `crates/spec-parser/src/lib.rs`, add your distro to the enum:

```rust
pub enum DistroFamily {
    OmniOS,
    Ubuntu,
    Fedora,   // New
}
```

Update the detection logic that maps the `distro` string to a family:

```rust
fn detect_family(distro: &str) -> DistroFamily {
    if distro.starts_with("ubuntu") {
        DistroFamily::Ubuntu
    } else if distro.starts_with("fedora") {
        DistroFamily::Fedora
    } else {
        DistroFamily::OmniOS
    }
}
```

## Step 2: Add Repository Type

If your distro uses a different repository format, add it to the `repositories` parsing in the spec:

```kdl
repositories {
    // Existing
    publisher name="..." origin="..."    // IPS
    apt-mirror "..." suite="..." components="..."  // APT

    // New: DNF/YUM
    dnf-repo name="fedora" baseurl="https://..." gpgkey="..."
}
```

## Step 3: Implement Phase 1 Logic

In `crates/forge-engine/src/phase1/mod.rs`, add the rootfs assembly path for your distro. This is the core work — each distro has its own bootstrap process:

- **OmniOS**: `pkg image-create` → set publishers → install
- **Ubuntu**: `debootstrap` → write sources.list → apt install
- **Fedora**: `dnf --installroot` → write repo files → dnf install

The key operations:
1. Initialize a package manager root in the staging directory
2. Configure repositories/mirrors
3. Install the base package set
4. Install user-specified packages

## Step 4: Default Builder Image

In `crates/forge-builder/src/lib.rs`, add a default builder image:

```rust
match distro_family {
    DistroFamily::OmniOS => "oci://ghcr.io/cloudnebulaproject/omnios-builder:latest",
    DistroFamily::Ubuntu => "oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest",
    DistroFamily::Fedora => "oci://ghcr.io/cloudnebulaproject/fedora-builder:latest",
}
```

You'll also need to build and publish the builder image itself.

## Step 5: Add Example Specs

Create example specs in the `images/` directory showing common patterns for the new distro.

## Step 6: Document

Add a chapter to this book under **Distro Guide** covering:
- Required and recommended packages
- Repository configuration
- Filesystem and bootloader defaults
- Any distro-specific overlay patterns

## Design Principles

When adding a distro, keep these principles in mind:

- **The `ToolRunner` trait** wraps all external tool execution. Use it for any new package manager commands — this enables testing without root access.
- **Phase 1 is distro-specific, Phase 2 is not**. The QCOW2/OCI/artifact target production is shared across all distros. Only rootfs assembly changes.
- **Filesystem defaults** should match what the distro community expects (ZFS for illumos, ext4 for most Linux).
- **Error messages** should use miette diagnostics to tell the user exactly what's wrong and how to fix it.
