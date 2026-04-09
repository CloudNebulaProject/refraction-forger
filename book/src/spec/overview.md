# Spec Overview

Forger uses [KDL](https://kdl.dev/) (KDL Document Language) as its specification format. KDL is a node-based document language that's more expressive than TOML or JSON while being more readable than YAML.

## Why KDL?

- **Node-based**: natural fit for declaring image components (packages, files, targets)
- **Arguments and properties**: nodes can have positional arguments and named properties on the same line
- **Children blocks**: nested structure without deep indentation
- **Type annotations**: optional type safety
- **Comments**: `//` line comments and `/* */` block comments

## Spec File Structure

A complete spec file has these top-level sections:

```kdl
// Identity
metadata name="my-image" version="1.0.0" description="What this image is"

// Which OS (omit for OmniOS default)
distro "ubuntu-22.04"

// Composability
base "path/to/parent.kdl"
include "path/to/shared-steps.kdl"

// Package sources
repositories {
    // IPS publishers or APT mirrors
}

// IPS-specific
incorporation "entire"
variants { /* ... */ }
certificates { /* ... */ }

// What to install
packages {
    package "pkg-name"
}

// Files and directories to place
overlays {
    // file, ensure-dir, ensure-symlink, shadow, devfsadm, remove-files
}

// Users and system setup
customization {
    user "username"
}

// Builder VM config (optional)
builder {
    image "oci://..."
    vcpus 4
    memory 4096
}

// What to produce
target "name" kind="qcow2" {
    // target-specific settings
}
```

All sections are optional except that you need at least one `target` to build anything.

## File Resolution

Paths in `base`, `include`, and overlay `source` fields are resolved relative to the spec file's directory. This means you can organize specs and their supporting files together:

```
images/
  omnios-base.kdl
  omnios-disk.kdl        # base "omnios-base.kdl"
  common.kdl
  devfs.kdl
  files/
    etc/
      hosts
      resolv.conf
      sshd_config
    omniosce-ca.cert.pem
```

## Validation

Always validate before building:

```bash
forger validate --spec my-image.kdl
```

This checks:
- KDL syntax correctness
- Schema conformance (required fields, valid types)
- Include/base resolution (files exist, no circular references)
