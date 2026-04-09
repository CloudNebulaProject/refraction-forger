# Remote Builder VMs

When the build host doesn't match the target OS, Forger delegates to an ephemeral builder VM. This chapter explains the internals.

## Lifecycle

```
1. Image Resolution
   ├── OCI reference → pull from registry
   ├── URL → download
   └── Local path → use directly

2. Cloud-Init Generation
   ├── Generate ephemeral Ed25519 SSH keypair
   ├── Create user-data: builder user, SSH key, passwordless sudo
   └── Create cloud-init ISO

3. VM Creation
   ├── Create VmSpec (CPU, memory, disk, network)
   ├── Network: user-mode (SLIRP) — no root required
   ├── Disk: overlay on builder image (20GB default)
   └── Hypervisor: auto-detect via vm-manager

4. Boot & Connect
   ├── Start VM via hypervisor
   └── SSH retry loop (up to 120 seconds)

5. Transfer
   ├── Upload forger binary via SCP
   ├── Upload spec files via SCP
   └── Upload overlay files via SCP

6. Build
   └── Execute forger build command via SSH

7. Download
   └── Retrieve artifacts via SCP

8. Teardown (guaranteed, even on failure)
   └── Destroy VM via hypervisor
```

## User-Mode Networking

Builder VMs use QEMU's user-mode networking (SLIRP). This means:

- **No root access** needed on the host
- **No bridge interfaces** to configure
- **No firewall rules** to manage
- Guest can access the internet through NAT
- Host communicates with guest via port forwarding

DNS is automatically configured if needed.

## Security Model

- SSH keys are **ephemeral** — generated per build, discarded after
- The builder user has **passwordless sudo** but only exists for the build duration
- The VM is **destroyed** after every build, even on failure
- No persistent state from previous builds leaks into new ones

## Custom Builder Images

Builder images are QCOW2 files with cloud-init support. To create your own:

1. Build a base image with Forger (or any tool)
2. Ensure `cloud-init` is installed and enabled
3. Ensure SSH server is running
4. Include all build dependencies (`pkg`, `qemu-img`, `zfs`, etc.)
5. Push to an OCI registry or host as a downloadable file

### Minimum Requirements for a Builder Image

- Cloud-init (for SSH key injection and user setup)
- SSH server
- The target distro's package manager
- `qemu-img` (for QCOW2 conversion)
- Filesystem tools (`zfs`/`zpool` for ZFS, `parted`/`mkfs.ext4` for ext4)
- Bootloader tools (GRUB, UEFI support)

## Disk Overlay

The builder VM uses a disk overlay (copy-on-write layer) on top of the builder image. This means:

- The original builder image is never modified
- The overlay provides additional working space (20GB by default)
- Multiple builds can run concurrently from the same builder image

## Troubleshooting

### VM Fails to Boot

- Check that the builder image has cloud-init enabled
- Verify QEMU is installed and accessible
- Check system resources (enough RAM and disk for the VM)

### SSH Connection Times Out

- The 120-second timeout may be insufficient for slow storage
- Check that the builder image's SSH server starts on boot
- Verify user-mode networking isn't blocked by firewall

### Build Fails Inside VM

- The error output from the remote build is captured and displayed
- Check that the builder image has all required tools installed
- For disk space issues, increase the disk overlay size in the builder config
