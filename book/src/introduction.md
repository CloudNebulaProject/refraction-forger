# Refraction Forger

Refraction Forger is a declarative image building tool that creates optimized OS images from simple specification files. It is designed for infrastructure engineers, DevOps teams, and OS distribution maintainers who need reproducible, cacheable image builds across multiple platforms.

## What It Does

Forger reads a `.kdl` specification file that declares what your image should contain — packages, files, users, boot configuration — and produces ready-to-deploy artifacts:

- **QCOW2 virtual machine images** with ZFS or ext4 filesystems
- **OCI container images** for container runtimes
- **Tar archives** for further processing or embedding

Built artifacts can be pushed directly to OCI registries like GHCR.

## Why Forger?

Traditional image building tools fall into two camps:

1. **Shell script orchestrators** (like the illumos `image-builder`) that require a matching host OS, root privileges, and careful manual sequencing.
2. **ISO boot automators** (like HashiCorp Packer) that spin up a full installer, type keystrokes into a virtual console, and wait — slowly.

Forger takes a different approach. It assembles images *directly* by calling package managers and filesystem tools, skipping the installer entirely. Builds that took 20+ minutes with Packer complete in a fraction of the time.

When your host doesn't match the target OS, Forger automatically spins up an ephemeral builder VM, transfers the spec, builds inside it, and retrieves the artifacts — no manual VM management needed.

## Primary Focus

Forger's primary focus is **illumos** distributions, particularly **OmniOS**. The illumos ecosystem uses IPS (Image Packaging System) and ZFS, both of which Forger understands natively.

Linux support (starting with **Ubuntu**) is included as a secondary target. The long-term goal is to bring IPS to Linux via a Rust implementation, making Forger's packaging model available across operating systems. In the meantime, popular Linux distributions are supported to provide a broad userbase familiar with tools like Packer.

## How This Book Is Organized

- **Getting Started** walks you through installation and building your first image.
- **The KDL Spec Language** is a complete guide to writing image specifications.
- **Composability** explains how to chain builds, share configuration, and create variants.
- **Distro Guide** covers distribution-specific details for illumos and Linux.
- **Output Formats** describes each artifact type and how to deploy them.
- **Architecture** explains the internal design for contributors and advanced users.
- **Migration** helps you move from `omnios-image-builder` or Packer.
- **Reference** provides CLI help, spec schema, and complete examples.
