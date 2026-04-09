# From Packer (oi-packer)

This guide helps you migrate from HashiCorp Packer-based illumos image building to Forger.

## Why Migrate?

| Packer | Forger |
|---|---|
| Boots ISO, types keystrokes, waits | Direct package manager assembly |
| 20+ minute builds | Minutes |
| No native caching | Multi-stage caching via `base` |
| Shell provisioners (order-sensitive) | Declarative KDL spec |
| Vagrant boxes, raw disks | QCOW2, OCI, tar artifacts |
| External upload scripts | Built-in OCI registry push |

## Mapping Concepts

### Packer Source → Not Needed

Packer sources define the VM that runs the installer:

```hcl
source "qemu" "oi-hipster" {
  iso_url          = "https://dlc.openindiana.org/isos/..."
  iso_checksum     = "sha256:..."
  disk_size        = "51200M"
  memory           = 4096
  accelerator      = "kvm"
}
```

Forger doesn't boot an ISO. It calls the package manager directly, so there's no installer VM to configure.

### Boot Command → Not Needed

Packer's boot command types keystrokes into the installer:

```hcl
boot_command = [
  "<wait10><wait10><wait10>",
  "47<enter><wait>",    // Select installation
  "7<enter><wait>",     // Select bootloader
  // ... dozens more keystrokes
]
```

Forger skips the installer entirely. Package installation happens through direct `pkg` or `apt` calls.

### Shell Provisioners → KDL Blocks

**Packer** (shell scripts that SSH into the VM):

```hcl
provisioner "shell" {
  scripts = [
    "scripts/update.sh",      // pkg update
    "scripts/vagrant.sh",     // Create vagrant user, install SSH keys
    "scripts/cleanup.sh",     // Remove SSH host keys, clear logs
  ]
}
```

**Forger** (declarative):

```kdl
packages {
    package "/editor/vim"
    package "/network/openssh-server"
}

customization {
    user "deploy"
}

overlays {
    file destination="/home/deploy/.ssh/authorized_keys" source="files/authorized_keys"
    shadow username="deploy" password="$5$..."
}
```

### Post-Processors → Targets

**Packer** (Vagrant box post-processor):

```hcl
post-processor "vagrant" {
  compression_level = 9
  output            = "OI-hipster-{{.Provider}}.box"
}
```

**Forger** (target block):

```kdl
target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    push-to "ghcr.io/myorg/my-image:latest"
}
```

Forger doesn't produce Vagrant boxes directly. Instead, it produces QCOW2 images that work with QEMU/KVM, Proxmox, and other hypervisors. OCI container images are also available.

### Variables → Profiles

**Packer** (HCL variables):

```hcl
variable "build_version" { default = "20240426" }
```

**Forger** (profiles for variants):

```kdl
packages if="dev" {
    package "/diagnostic/top"
}
```

## Full Migration Example

### Packer Template (before)

```hcl
source "qemu" "oi-hipster" {
  iso_url     = "https://dlc.openindiana.org/isos/hipster/20240426/OI-hipster-text-20240426.iso"
  disk_size   = "51200M"
  memory      = 4096
  format      = "qcow2"
  boot_command = ["<wait30>", "47<enter>", /* ... 30 more lines ... */]
}

build {
  sources = ["source.qemu.oi-hipster"]

  provisioner "shell" { scripts = ["scripts/update.sh"] }
  provisioner "shell" { scripts = ["scripts/vagrant.sh"] }
  provisioner "shell" { scripts = ["scripts/cleanup.sh"] }

  post-processor "vagrant" {
    output = "OI-hipster-{{.Provider}}.box"
  }
}
```

### Forger Spec (after)

```kdl
metadata name="oi-hipster" version="1.0.0" description="OpenIndiana Hipster image"

repositories {
    publisher name="openindiana.org" origin="http://pkg.openindiana.org/hipster/"
}

incorporation "entire"

variants {
    set name="opensolaris.zone" value="global"
}

packages {
    package "/editor/vim"
    package "/network/openssh-server"
    package "/network/rsync"
    package "/driver/network/vioif"
    package "/driver/storage/vioblk"
}

customization {
    user "deploy"
}

overlays {
    file destination="/home/deploy/.ssh/authorized_keys" source="files/authorized_keys"
}

target "vm" kind="qcow2" {
    disk-size "8G"
    bootloader "uefi"
    filesystem "zfs"
    push-to "ghcr.io/myorg/oi-hipster:latest"
}
```

Build time drops from 20+ minutes (ISO boot + install + provisioning) to a few minutes (direct package installation).

## Migration Checklist

1. Identify packages installed by your shell provisioners
2. List files copied or modified by provisioners
3. Translate to KDL `packages`, `overlays`, and `customization` blocks
4. Replace ISO-based installation with `repositories` + `incorporation`
5. Replace Vagrant post-processor with QCOW2 target + OCI push
6. Test with `forger validate` and `forger inspect`
7. Build and verify the image boots correctly
