# Two-Phase Build Pipeline

Every Forger build follows a two-phase architecture. Understanding this split is key to building efficient images and debugging build failures.

## Phase 1: Rootfs Assembly

Phase 1 creates a populated filesystem tree in a staging directory. The work is entirely distro-specific:

### OmniOS Path

1. `pkg image-create` — Initialize an IPS package image
2. `pkg set-publisher` — Configure each publisher (name + origin URL)
3. `pkg change-variant` — Set zone variant (`global` or `nonglobal`)
4. `pkg approve-ca-cert` — Trust CA certificates for signed packages
5. `pkg install` — Install all specified packages
6. Apply customizations (create users)
7. Apply overlays (files, directories, symlinks, shadow passwords, devfsadm)

### Ubuntu Path

1. `debootstrap` — Bootstrap a minimal Debian/Ubuntu rootfs
2. Write `/etc/apt/sources.list` from repository configuration
3. `apt update` — Refresh package lists
4. `apt install` — Install all specified packages
5. Apply customizations and overlays (same as OmniOS)

### Output

Phase 1 produces a staging directory containing a complete rootfs. This directory is consumed by Phase 2.

## Phase 2: Target Production

Phase 2 takes the rootfs from Phase 1 and packages it into the requested output format. This logic is **shared across all distros**.

### QCOW2 Path

```
Create raw disk file (specified size)
  → Attach as loopback device
    → ZFS: create pool → create BE dataset → mount
    → ext4: partition → format → mount
      → Copy rootfs into mounted filesystem
        → Install bootloader (UEFI/GRUB)
          → ZFS: set bootfs → unmount → export pool
          → ext4: unmount
            → Detach loopback
              → qemu-img convert raw → qcow2
```

The ZFS path creates a unique pool name during build (e.g., `forgebuild_12345`) and renames to `rpool` after export. This prevents conflicts with existing pools on the build host.

### OCI Path

```
Compress rootfs → tar.gz layer
  → Compute SHA256 digest
    → Build OCI config JSON (entrypoint, env)
      → Build OCI manifest JSON
        → Write OCI Image Layout directory
```

### Artifact Path

```
Create tar archive from rootfs
```

## Why Two Phases?

The split serves several purposes:

1. **Separation of concerns**: Distro-specific logic (Phase 1) doesn't leak into format-specific logic (Phase 2)
2. **Multiple targets**: One Phase 1 rootfs can produce QCOW2, OCI, and artifact targets without rebuilding
3. **Caching boundary**: The base/child relationship creates a cache point between Phase 1 and Phase 2
4. **Testability**: Each phase can be tested independently

## Error Recovery

If Phase 2 fails (e.g., disk too small, bootloader installation error), Forger cleans up:

- Unmounts filesystems
- Exports ZFS pools
- Detaches loopback devices
- Removes temporary files

Cleanup runs even on failure, preventing resource leaks on the build host.
