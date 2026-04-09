# Packages

The `packages` block declares which packages to install in the image. Package names follow the conventions of the target distro's package manager.

## Basic Usage

```kdl
packages {
    package "/editor/vim"
    package "/network/openssh-server"
    package "/network/rsync"
    package "/service/network/ntpsec"
}
```

Each `package` node takes a single argument: the package name.

## IPS Package Names (OmniOS)

IPS uses hierarchical FMRI (Fault Management Resource Identifier) paths:

```kdl
packages {
    package "/editor/vim"                    // Editors
    package "/network/openssh-server"        // Network services
    package "/ooce/developer/rust"           // OmniOS Community Extra (OOCE)
    package "/developer/build/gnu-make"      // Build tools
    package "/driver/virtio/vioif"           // Virtio network driver
}
```

The leading `/` is conventional for IPS. You can find available packages with `pkg search` on an OmniOS system or browse [pkg.omnios.org](https://pkg.omnios.org).

## APT Package Names (Ubuntu)

Ubuntu uses flat package names:

```kdl
packages {
    package "build-essential"
    package "curl"
    package "git"
    package "openssh-server"
    package "linux-image-generic"
}
```

## Conditional Packages (Profiles)

Packages can be scoped to a [profile](../composability/profiles.md) using the `if` property. They're only installed when that profile is active:

```kdl
// Always installed
packages {
    package "/editor/vim"
    package "/network/openssh-server"
}

// Only when building with --profile build
packages if="build" {
    package "/developer/build-essential"
    package "/ooce/developer/omnios-build-tools"
    package "/developer/build/gnu-make"
}
```

Build with a profile:

```bash
forger build --spec my-image.kdl --profile build
```

Multiple `packages` blocks with different `if` conditions can coexist in a single spec.

## IPS Incorporations

For OmniOS, the `incorporation` node pins the entire OS to a consistent version set:

```kdl
incorporation "entire"
```

This ensures all installed packages are compatible. The `entire` incorporation is standard for OmniOS and should almost always be included.

## IPS Variants

Variants control which facets of packages are installed:

```kdl
variants {
    set name="opensolaris.zone" value="global"
}
```

Setting `opensolaris.zone` to `"global"` ensures you get the full global-zone package set, including kernel modules and boot components. Use `"nonglobal"` for zone-only images.
