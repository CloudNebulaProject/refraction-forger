# Design Overview

Refraction Forger is designed around three principles: **declarative specs**, **two-phase execution**, and **distro abstraction**.

## High-Level Architecture

```
                    ┌─────────────┐
                    │  KDL Spec   │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ spec-parser │  Parse → Resolve → Filter
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   forger    │  CLI routing
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │                         │
       ┌──────▼──────┐          ┌──────▼──────┐
       │ Local Build  │          │forge-builder│
       └──────┬──────┘          └──────┬──────┘
              │                         │
              │                  Ephemeral VM
              │                  ┌─────────────┐
              │                  │  SSH + SCP   │
              │                  └──────┬──────┘
              │                         │
       ┌──────▼─────────────────────────▼──────┐
       │            forge-engine                │
       │  ┌──────────┐     ┌─────────────────┐ │
       │  │ Phase 1   │────▶│    Phase 2      │ │
       │  │ Rootfs    │     │ QCOW2/OCI/Tar  │ │
       │  └──────────┘     └────────┬────────┘ │
       └────────────────────────────┼──────────┘
                                    │
                             ┌──────▼──────┐
                             │  forge-oci  │  Registry push
                             └─────────────┘
```

## Key Design Decisions

### Declarative Over Imperative

The old tools used shell scripts to orchestrate builds — ordering mattered, error handling was manual, and reuse required copy-paste. Forger uses a declarative spec where you describe *what* the image should contain, and the engine handles *how*.

### Direct Assembly Over Installation

Packer boots an ISO, types keystrokes into a virtual console, and waits for an installer to finish. Forger calls the package manager directly to assemble a rootfs, skipping the installer entirely. This is faster and more reliable.

### Host Independence

The old `omnios-image-builder` required an illumos host with ZFS and pfexec. Forger can build from any platform by spinning up an ephemeral builder VM. The build environment is part of the spec, not a prerequisite.

### OCI as Distribution Channel

Instead of custom upload scripts for each cloud provider, Forger uses OCI registries as a universal distribution mechanism. VM images, container images, and tar artifacts all flow through the same registry infrastructure.

## Error Handling

Forger uses [miette](https://docs.rs/miette) for rich error diagnostics. When something fails, you get:

- The error message
- Context about what was being attempted
- Suggestions for how to fix it
- Source location where the error originated

This is deliberate — image building involves many external tools and system operations, and clear error messages are essential for debugging.
