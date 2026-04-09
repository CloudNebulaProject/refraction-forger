# Customizations

The `customization` block configures users and system-level settings.

## Users

Create a user in the image:

```kdl
customization {
    user "ci"
}
```

| Property | Required | Description |
|---|---|---|
| *(argument)* | Yes | Username to create |

The user is created with default settings appropriate for the target distro.

## Conditional Customizations

Like packages and overlays, customization blocks support profile filtering:

```kdl
customization if="ci" {
    user "ci"
}
```

This user is only created when the `ci` profile is active:

```bash
forger build --spec my-image.kdl --profile ci
```

## Combined with Overlays

For full user setup, combine customization with overlay operations:

```kdl
customization {
    user "deploy"
}

overlays {
    // Set user password
    shadow username="deploy" password="$5$..."

    // Ensure SSH directory
    ensure-dir "/home/deploy/.ssh" owner="deploy" group="staff" mode="700"

    // Deploy authorized keys
    file destination="/home/deploy/.ssh/authorized_keys" source="files/authorized_keys"
}
```
