# Profiles (Conditional Variants)

Profiles let you create multiple image variants from a single spec. Blocks tagged with `if="profile-name"` are only included when that profile is active.

## Syntax

Tag any `packages`, `overlays`, or `customization` block with an `if` property:

```kdl
// Always included (no condition)
packages {
    package "/editor/vim"
    package "/network/openssh-server"
}

// Only when --profile build is active
packages if="build" {
    package "/developer/build-essential"
    package "/ooce/developer/omnios-build-tools"
}

// Only when --profile ci is active
customization if="ci" {
    user "ci"
}

overlays if="debug" {
    file destination="/etc/system" source="files/etc/system.debug"
}
```

## Activating Profiles

Use the `--profile` flag (repeatable) when building or inspecting:

```bash
# No profiles — just the base packages
forger build --spec my-image.kdl

# With build tools
forger build --spec my-image.kdl --profile build

# With build tools AND CI user
forger build --spec my-image.kdl --profile build --profile ci

# Inspect with profiles to see what would be included
forger inspect --spec my-image.kdl --profile build --profile ci
```

## How Filtering Works

Profile filtering happens after spec resolution (base + include merging) but before the build starts:

1. Parse and resolve the full spec (with all blocks)
2. Apply profile filter:
   - Blocks with **no `if`** condition → always kept
   - Blocks where `if` value **matches** an active profile → kept
   - Blocks where `if` value **doesn't match** any active profile → removed
3. Build proceeds with the filtered spec

## Use Cases

### Development vs Production

```kdl
packages {
    package "/network/openssh-server"
}

packages if="dev" {
    package "/developer/build-essential"
    package "/diagnostic/top"
}

overlays if="dev" {
    shadow username="root" password="$5$..."
}
```

### CI Variants

```kdl
packages if="rust-ci" {
    package "/ooce/developer/rust"
    package "/developer/build/gnu-make"
}

packages if="go-ci" {
    package "/ooce/developer/go"
}
```

### Cloud Provider Specific

```kdl
overlays if="aws" {
    file destination="/boot/conf.d/console" source="files/boot_console.aws"
    file destination="/etc/dhcp/dhcpagent" source="files/dhcpagent.aws"
}

overlays if="digitalocean" {
    file destination="/boot/conf.d/console" source="files/boot_console.do"
}
```

## Combining with Base and Includes

Profiles work across the full composition chain. Conditional blocks in base specs and includes are filtered together with the current spec's blocks:

```kdl
// omnios-base.kdl
packages if="build" {
    package "/developer/build-essential"
}
```

```kdl
// my-image.kdl
base "omnios-base.kdl"

packages if="build" {
    package "/ooce/developer/rust"
}
```

Building with `--profile build` activates both blocks from both files.
