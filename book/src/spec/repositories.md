# Repositories & Publishers

The `repositories` block defines where packages are fetched from. The syntax differs between IPS (OmniOS) and APT (Ubuntu).

## IPS Publishers (OmniOS)

OmniOS uses IPS publishers — named package repositories with a URL origin:

```kdl
repositories {
    publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
    publisher name="extra.omnios" origin="https://pkg.omnios.org/bloody/extra/"
}
```

| Property | Required | Description |
|---|---|---|
| `name` | Yes | Publisher name (e.g., `"omnios"`, `"extra.omnios"`) |
| `origin` | Yes | Repository URL |

### Common OmniOS Publishers

| Name | Origin | Contents |
|---|---|---|
| `omnios` | `https://pkg.omnios.org/bloody/core/` | Core OS packages (bloody) |
| `extra.omnios` | `https://pkg.omnios.org/bloody/extra/` | Additional packages (bloody) |
| `omnios` | `https://pkg.omnios.org/r151050/core/` | Core OS packages (stable) |

### Publisher Verification

OmniOS publishers use signed packages. Configure CA certificates in the [`certificates`](./repositories.md#certificates) block to enable verification:

```kdl
certificates {
    ca publisher="omnios" certfile="omniosce-ca.cert.pem"
}
```

The `certfile` path is resolved relative to the spec file's directory.

## APT Mirrors (Ubuntu)

Ubuntu uses APT repositories with suite and component selection:

```kdl
repositories {
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy" components="main universe"
}
```

| Property | Required | Description |
|---|---|---|
| *(argument)* | Yes | Mirror URL |
| `suite` | Yes | Distribution codename (e.g., `"jammy"`) |
| `components` | Yes | Space-separated component list (e.g., `"main universe"`) |

### Common Ubuntu Mirrors

```kdl
repositories {
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy" components="main universe"
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy-updates" components="main universe"
    apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy-security" components="main universe"
}
```

## Deduplication

When specs are composed via `base` or `include`, repositories are merged by name (IPS) or URL (APT). Duplicate entries are deduplicated automatically.
