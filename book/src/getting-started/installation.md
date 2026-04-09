# Installation

## Prerequisites

Forger is written in Rust and builds as a single static binary. You need:

- **Rust toolchain** (1.85 or later) via [rustup](https://rustup.rs/)
- **Git** for fetching the source

For **local builds** (building images directly on your machine), you also need:

- The target OS's package manager available (e.g., `pkg` on OmniOS, `debootstrap` + `apt` on Ubuntu)
- Root/sudo access for filesystem operations
- `qemu-img` for QCOW2 conversion
- ZFS utilities if building ZFS-based images

For **remote builds** (the default when your host doesn't match the target), you only need:

- QEMU installed (for the builder VM)
- No root access required — Forger uses user-mode networking

## Building From Source

```bash
git clone https://github.com/cloudnebulaproject/refraction-forger.git
cd refraction-forger
cargo build --release
```

The binary is at `target/release/forger`.

### Cross-Compilation for illumos

If you're building on Linux for deployment on illumos:

```bash
# Install the cross-compilation tool
cargo install cross

# Build for illumos
cross build --release --target x86_64-unknown-illumos
```

The project includes a `Cross.toml` with the illumos target preconfigured.

## Verify Installation

```bash
forger --help
```

You should see the five subcommands: `build`, `validate`, `inspect`, `push`, and `targets`.
