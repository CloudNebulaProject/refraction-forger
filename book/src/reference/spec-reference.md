# KDL Spec Reference

Complete reference for all KDL spec nodes and properties.

## Top-Level Nodes

### metadata

```kdl
metadata name="<string>" version="<string>" description="<string>"
```

| Property | Type | Required | Description |
|---|---|---|---|
| `name` | string | Yes | Image name |
| `version` | string | Yes | Semantic version |
| `description` | string | No | Human-readable description |

### distro

```kdl
distro "<string>"
```

Distro identifier. Determines the build path (IPS vs APT). Omit for OmniOS (default).

| Value | Family |
|---|---|
| *(omitted)* | OmniOS |
| `"ubuntu-22.04"` | Ubuntu |

### base

```kdl
base "<path>"
```

Parent spec file. Creates a build-stage boundary for caching. Path is relative to the current spec file.

### include

```kdl
include "<path>"
```

Sibling spec file to merge. Imports shared steps. Path is relative to the current spec file.

### incorporation

```kdl
incorporation "<string>"
```

IPS incorporation constraint (OmniOS only). Typically `"entire"`.

## Block Nodes

### repositories

```kdl
repositories {
    publisher name="<string>" origin="<url>"
    apt-mirror "<url>" suite="<string>" components="<string>"
}
```

**publisher** (IPS):

| Property | Type | Required |
|---|---|---|
| `name` | string | Yes |
| `origin` | string (URL) | Yes |

**apt-mirror** (APT):

| Argument | Type | Required |
|---|---|---|
| *(positional)* | string (URL) | Yes |
| `suite` | string | Yes |
| `components` | string (space-separated) | Yes |

### certificates

```kdl
certificates {
    ca publisher="<string>" certfile="<path>"
}
```

| Property | Type | Required |
|---|---|---|
| `publisher` | string | Yes |
| `certfile` | string (path) | Yes |

### variants

```kdl
variants {
    set name="<string>" value="<string>"
}
```

IPS variant settings. Common: `name="opensolaris.zone" value="global"`.

### packages

```kdl
packages if="<profile>" {
    package "<name>"
}
```

| Property | Type | Required |
|---|---|---|
| `if` | string | No |

The `if` property enables profile filtering. Omit for unconditional packages.

### overlays

```kdl
overlays if="<profile>" {
    file destination="<path>" source="<path>"
    ensure-dir "<path>" owner="<string>" group="<string>" mode="<string>"
    ensure-symlink "<path>" target="<path>"
    remove-files "<path>" "<path>" ...
    shadow username="<string>" password="<string>"
    devfsadm
}
```

**file**:

| Property | Type | Required |
|---|---|---|
| `destination` | string (absolute path) | Yes |
| `source` | string (relative path) | Yes |

**ensure-dir**:

| Argument/Property | Type | Required |
|---|---|---|
| *(positional)* | string (path) | Yes |
| `owner` | string | No |
| `group` | string | No |
| `mode` | string (octal) | No |

**ensure-symlink**:

| Argument/Property | Type | Required |
|---|---|---|
| *(positional)* | string (link path) | Yes |
| `target` | string (target path) | Yes |

**remove-files**: One or more path arguments.

**shadow**:

| Property | Type | Required |
|---|---|---|
| `username` | string | Yes |
| `password` | string (crypt hash) | Yes |

**devfsadm**: No arguments.

### customization

```kdl
customization if="<profile>" {
    user "<username>"
}
```

### builder

```kdl
builder {
    image "<string>"
    vcpus <integer>
    memory <integer>
    disk <integer>
}
```

| Property | Type | Required | Default |
|---|---|---|---|
| `image` | string | No | Distro default |
| `vcpus` | integer | No | Host-dependent |
| `memory` | integer (MB) | No | Host-dependent |
| `disk` | integer (GB) | No | 20 |

### target

```kdl
target "<name>" kind="<kind>" {
    // kind-specific properties
}
```

| Property | Type | Required |
|---|---|---|
| *(positional)* | string (name) | Yes |
| `kind` | string | Yes |

**kind="qcow2"**:

| Property | Type | Required |
|---|---|---|
| `disk-size` | string (e.g., "8G") | Yes |
| `bootloader` | string | Yes |
| `filesystem` | string | No |
| `push-to` | string (registry ref) | No |
| `pool` children | property nodes | No |

**kind="oci"**:

| Property | Type | Required |
|---|---|---|
| `entrypoint` | node with `command` | No |
| `environment` children | `set` nodes | No |
| `push-to` | string | No |

**kind="artifact"**: No additional properties.
