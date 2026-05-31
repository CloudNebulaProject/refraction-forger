# Builder Binary

When Forger builds without `--local`, it launches a builder VM and then needs
the matching `forger` binary *inside* that VM to drive Phase 2. By default
Forger looks for a cross-compiled binary in the workspace target directory (dev
builds) and otherwise downloads a published GitHub release asset.

That isn't always what you want. You may have release-installed Forger with no
workspace target, no published asset for your version, a binary you
cross-built on another machine, a private mirror because GitHub is unreachable,
or a desire to pin an exact sha256 for reproducibility. The `builder.binary`
mechanism lets any consumer point Forger at a `forger` binary of their choosing
without patching Forger itself.

## Resolver priority

Forger resolves the builder binary from the first source that produces bytes:

```
--builder-binary (CLI flag)
      ↓
builder { binary "…" }  (KDL spec block)
      ↓
FORGER_BUILDER_BINARY   (environment variable)
      ↓
dev fallback            (workspace target/<triple>/release/forger)
      ↓
release fallback        (GitHub release URL)
```

The env var sits *below* the spec block on purpose: the spec is the
version-controlled contract for the image, while env vars are ambient overrides
for when the spec is silent. To override a spec-configured binary on a single
invocation, use the CLI flag.

A configured source (flag, spec, or env var) that fails to resolve is an
**error** — Forger does not silently fall through to the dev/release fallbacks
once you've asked for something explicit.

## Source kinds

The source string's prefix selects the kind:

| Prefix | Kind | Notes |
|---|---|---|
| `oci://` | OCI artifact | Pulled from a registry (see below) |
| `http://` or `https://` | URL | Downloaded directly |
| `/…`, `./…`, `../…` | Local path | Copied (not symlinked) into the cache |
| anything else | — | Rejected |

Relative paths resolve against the **spec file's directory** for spec-block
sources, and against the **current working directory** for CLI/env sources.

### The `{triple}` token

`{triple}` is replaced with the builder's Rust target triple before any
filesystem or network access:

| Distro family | Triple |
|---|---|
| Ubuntu | `x86_64-unknown-linux-gnu` |
| OmniOS | `x86_64-unknown-illumos` |

So a single line serves both Linux and illumos builders:

```kdl
builder { binary "http://artifacts.lan/forger-{triple}" }
```

`{triple}` is the only recognized token. Pin a specific version with a
versioned URL or an `sha256`.

## sha256 pinning

`sha256` pins the binary content. Supply the bare hex, optionally with a
`sha256:` prefix (so you can paste digests from `oras`, `crane`, or
`docker manifest inspect`):

```kdl
builder { binary "https://artifacts.lan/forger-{triple}" sha256="9a3f…c0" }
```

On the CLI it's a separate, composable flag:

```bash
forger build --spec img.kdl \
  --builder-binary "https://artifacts.lan/forger-{triple}" \
  --builder-binary-sha256 9a3f…c0
```

- When set, the fetched bytes must match or Forger fails with a sha256 mismatch
  error and does **not** populate the cache.
- When the content-addressed cache entry already exists, it is re-hashed and a
  mismatch triggers an unlink + re-fetch (cache-poisoning protection).
- When omitted, Forger logs a warning on fetch suggesting you add one.

## Caching

Resolved binaries are cached under
`${XDG_CACHE_HOME or ~/.cache}/forger/builder-binaries/`:

- With an sha256 pin the entry is **content-addressed**:
  `forger-{triple}-sha256-{hex}`.
- Without one it is keyed by a sha1 of the post-`{triple}` source string:
  `forger-{triple}-{sha1}` (changing the URL invalidates the entry).

Fetches stream to a unique tempfile in the cache directory, are hashed and
verified, then atomically renamed into place — no partial files, and concurrent
writers are safe. Local-path sources are **copied** so the cache survives
deletion of the source. Every entry is `chmod 0755`.

## OCI artifacts

An `oci://` source must be a single-layer OCI **artifact**, not a container
image. Publish one with `oras`:

```bash
oras push ghcr.io/myorg/forger:linux-gnu \
  ./forger:application/vnd.refraction.forger.binary.v1
```

```kdl
builder { binary "oci://ghcr.io/myorg/forger:{triple}" sha256="…" }
```

- Acceptable layer media types: `application/vnd.refraction.forger.binary.v1`
  (preferred) or `application/octet-stream`.
- `application/vnd.oci.image.layer.*` / `…docker.image.rootfs.*` (tar / gzip)
  layers are rejected with a hint to republish as an artifact.
- A reference that resolves to a multi-platform image index is rejected; use a
  per-platform tag via `{triple}` instead.

### Authentication

Anonymous pulls work for public registries. For private ones, Forger reads
`${DOCKER_CONFIG:-~/.docker/config.json}` and uses the base64 `user:pass` from
`auths[<registry>].auth` (i.e. a plain `docker login`). Credential helpers
(`credsStore` / `credHelpers`) are out of scope; for those, set a pre-resolved
bearer token instead:

```bash
export FORGER_OCI_BEARER=eyJ…
```

## Environment variables

| Variable | Description |
|---|---|
| `FORGER_BUILDER_BINARY` | Source string (same syntax as the spec/flag) |
| `FORGER_BUILDER_BINARY_SHA256` | sha256 pin for the above |
| `FORGER_OCI_BEARER` | Pre-resolved bearer token for private OCI registries |

These are handy in CI where you don't want to write sources into every spec.

## Output naming (related)

Unrelated to the binary source but shipped alongside it: `--output-name`
overrides the `{target.name}.qcow2` (or `.tar.gz` / `-oci`) output base name.
It requires `--target` to disambiguate which artifact to rename:

```bash
forger build --spec img.kdl --target vm --output-name solstice-ubuntu-22.04
# -> output/solstice-ubuntu-22.04.qcow2
```
