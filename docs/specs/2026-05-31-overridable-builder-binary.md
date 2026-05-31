# Feature: Overridable Builder Forger Binary

Date: 2026-05-31
Status: Approved — ready to implement
Target branch: main (direct commits)

## Audience

This spec is self-contained for a fresh implementing agent. It describes the
*entire* feature, not an MVP. Where alternatives exist, the chosen path is
specified. Where the implementation must touch a file, the file is named.

## Problem

`forger build` (without `--local`) launches a builder VM and then needs the
matching `forger` binary inside that VM to drive Phase 2. Today's resolver
(`crates/forge-builder/src/binary.rs::resolve_forger_binary`) has two modes:

1. **Dev mode** (`is_dev_build()` true): look for a cross-compiled binary at
   `<workspace>/target/<triple>/release/forger`.
2. **Release mode**: download from a hard-coded GitHub release URL
   `https://github.com/CloudNebulaProject/refraction-forger/releases/download/v{version}/forger-{triple}`.

Both fail for users who:
- Don't have the workspace target on hand (release-installed forger).
- Don't have a published GitHub release asset for the running version.
- Want to serve a binary they cross-built on a different machine.
- Want to mirror binaries inside a private network where GitHub is unreachable.
- Want to pin to a specific sha256 for reproducibility / supply-chain hygiene.

The fix is a configurable, layered resolver: CLI flag → spec block → existing
fallbacks, with three source kinds (local path, HTTP(S) URL, OCI ref),
triple templating, and optional sha256 verification.

## Goals

1. Any consumer can point forger at a forger binary of their choosing without
   patching forger itself.
2. The mechanism is project-level (in the spec, version-controlled with the
   image) AND override-able per invocation (CLI flag).
3. Source kinds cover the three realistic delivery channels: local file,
   HTTP(S), OCI registry.
4. URL/OCI sources support sha256 pinning; cache keys are content-addressed
   when sha256 is provided.
5. The same `binary` line works for Linux and illumos builder VMs via a
   `{triple}` template token.
6. Backward-compatible: nothing breaks for existing specs that omit the new
   field.
7. Documented end-to-end (CLI help, mdbook, example spec snippets).

## Non-goals

- Building or publishing GitHub release assets (orthogonal — out of scope).
- Replacing the dev-mode workspace lookup (it stays as a final fallback).
- Cross-architecture builds inside the builder VM (assumes binary matches
  builder triple).
- Auth headers beyond a simple `Authorization` env-var passthrough (signed
  URLs work today via query-string).

## Architecture

### Resolver priority (highest first)

```
CLI flag (--builder-binary)
      ↓
KDL spec block (builder { binary "..." sha256="..." })
      ↓
Env var (FORGER_BUILDER_BINARY)
      ↓
Dev fallback (is_dev_build → workspace target dir)
      ↓
Release fallback (existing GitHub URL — kept for backward compat)
```

Env var sits below the spec block intentionally: the spec is the version-controlled contract for the image; env vars are ambient overrides for when the spec is silent. To override a spec-configured binary, use the CLI flag.

The first source that *resolves to bytes on disk* wins. A configured source
that fails to download is an error, not a fall-through — we don't silently
fall back when the user explicitly asked for something.

### Source kinds

```rust
pub enum BinarySource {
    /// Local file. Copied (not symlinked) into the cache.
    Path(PathBuf),

    /// http://... or https://...
    Url { url: String, sha256: Option<String> },

    /// oci://registry/repo:tag or oci://registry/repo@sha256:...
    /// Pulls a single-layer image; binary is the layer payload.
    Oci { reference: String, sha256: Option<String> },
}
```

Source parsing rules:

| Input prefix | Kind | Notes |
|--------------|------|-------|
| `oci://` | `Oci` | strip prefix |
| `http://` or `https://` | `Url` | accept as-is |
| starts with `/` or `./` or `../` | `Path` | resolve relative to spec file's dir for path-relative; absolute for absolute |
| anything else | error | reject ambiguous inputs (`Use a /path/, http(s)://, or oci://` message) |

`{triple}` substitution applies uniformly to all three source kinds (Path, Url, Oci). It is performed on the source string immediately after kind detection and before any filesystem or network access, using the distro-resolved triple.

`{triple}` substitution happens after parsing, before fetch, against the
distro-family-resolved triple (`x86_64-unknown-linux-gnu` for Ubuntu,
`x86_64-unknown-illumos` for OmniOS). Substitution is literal string
replacement; only `{triple}` is recognized (no `{version}` token — version
pinning is done via sha256 or a versioned URL written by the user).

### Cache strategy

- Cache root unchanged: `${XDG_CACHE_HOME or ~/.cache}/forger/builder-binaries/`.
- Cache filename:
  - If sha256 provided: `forger-{triple}-sha256-{sha256-hex}` (content-addressed).
  - Otherwise: `forger-{triple}-{sha1-of-source-string}` (best-effort dedupe).
  - The sha1 is computed over the POST-`{triple}`-substitution source string (after the triple is interpolated) so that two distinct concrete URLs sharing a template do not collide and any change to the URL invalidates the cache key.
  - The existing `forger-{triple}-v{version}` layout for the release fallback is unchanged.
- On every URL/OCI fetch: stream to a uniquely-named tempfile in the cache directory (use `tempfile::NamedTempFile::new_in(cache_dir)` or an equivalent unique-suffix scheme — never a single shared `.tmp` sibling), hash, verify against the configured sha256 (if any), then `rename(2)` atomically over the final content-addressed name. Concurrent writers each produce their own tempfile; the last `rename` wins and, because the destination name is content-addressed when sha256 is configured, all winners write identical bytes. No partial files in the cache.
- Local-path source: copy to cache (don't symlink — cache must survive
  source deletion) using the same naming rule.
- Always chmod 0755 on the cache entry.

### sha256 verification

- When sha256 is configured AND the content-addressed cache entry already exists: re-hash on first resolve in this process; cache the verification result for the lifetime of the resolver call (typical cost: ~30 MB binary, <500 ms on commodity hardware). Mismatch ⇒ unlink and re-fetch. For non-content-addressed cache entries (sha1-of-source-string form, no user sha256 configured) no integrity check is possible on cache hit; the omitted-sha256 warning fires only on fetch, not on cache hit.
- When sha256 is configured AND the fetched bytes don't match: discard the
  download, surface a `BinarySha256Mismatch { expected, got, url }` error.
- When sha256 is omitted: log a warning at INFO level on every fetch
  (`forger binary fetched without sha256 pin; consider adding one`).

### CLI flag

Add `--builder-binary <source>` to the `build` subcommand only in
`crates/forger/src/main.rs` (alongside `--builder-image`). Threading:

`main.rs::Build::builder_binary` (`Option<String>`) → `commands::build::run(..., builder_binary: Option<&str>)` → `forge_builder::run_in_builder(..., builder_binary: Option<&str>, spec_path: &Path)` → `binary::resolve_forger_binary(distro, source: Option<&BinarySource>, spec_dir: Option<&Path>)`. Parsing of the source string into `BinarySource` happens at the resolver call site that has `spec_path` in hand. Base-directory semantics: CLI- and env-var-supplied relative paths resolve against the current working directory; spec-block paths resolve against the spec file's directory.

For sha256 on the CLI form: support `--builder-binary <source>` with
`--builder-binary-sha256 <hex>` as a separate flag. (Avoid encoding sha256
inline in the URL — keeps the URL readable and the flag composable.)

### Env var

`FORGER_BUILDER_BINARY=<source>`, `FORGER_BUILDER_BINARY_SHA256=<hex>`, and
`FORGER_OCI_BEARER=<token>` (pre-resolved bearer for private OCI registries),
checked after CLI and spec but before the existing dev/release fallbacks.
Same source-string rules as the CLI flag. Useful for CI environments that
don't want to write sources into every spec.

### Spec block

Extend `BuilderNode` in `crates/spec-parser/src/schema.rs`:

```rust
#[derive(Debug, Decode)]
pub struct BuilderNode {
    #[knuffel(child, unwrap(argument))]
    pub image: Option<String>,

    #[knuffel(child)]
    pub binary: Option<BuilderBinary>,        // NEW

    #[knuffel(child, unwrap(argument))]
    pub vcpus: Option<u16>,

    #[knuffel(child, unwrap(argument))]
    pub memory: Option<u64>,

    #[knuffel(child, unwrap(argument))]
    pub disk: Option<u32>,
}

#[derive(Debug, Decode)]
pub struct BuilderBinary {
    #[knuffel(argument)]
    pub source: String,
    #[knuffel(property)]
    pub sha256: Option<String>,
}
```

KDL usage:

```kdl
builder {
    image "https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img"
    binary "http://artifacts.lan/forger-{triple}" sha256="9a3f...c0"
    vcpus 4
    memory 4096
    disk 20
}
```

Spec resolution (`crates/spec-parser/src/resolve.rs`) must:
- Treat `builder.binary` as a leaf (no merging — child specs override
  parent's `binary` block atomically, matching how `image` works).
- Pass through include layering without flattening.

### OCI source

Use the `oci-client` crate (formerly `oci-distribution`, renamed under oras-project — use the new name) (already in the broader ecosystem; if not
yet a dependency, add it). Behavior:

- Parse `oci://registry/repo:tag` or `oci://registry/repo@sha256:...`.
- Pull manifest; require exactly one layer (otherwise reject as
  malformed-for-forger with a clear message).
- Stream the layer to the cache `.tmp` path, hash, verify against the
  user-provided `sha256`. Under the artifact contract (uncompressed `application/octet-stream` or `vnd.refraction.forger.binary.v1` layer) this equals the layer descriptor digest from the manifest, so users may copy-paste digests from `oras discover`, `crane digest`, or `docker manifest inspect`. The verification is therefore redundant in the happy path but cheap, and remains the correct check if a future media type ever introduces compression.
- Anonymous pull by default. For private registries, forger parses `${DOCKER_CONFIG:-~/.docker/config.json}` and extracts `auths[<registry>].auth` (base64 user:pass) into `RegistryAuth::Basic`. Credential helpers (`credsStore` / `credHelpers`, e.g. `docker-credential-gcloud`) are OUT OF SCOPE for v1 — for those, users must either run `docker login` with a plain password (writes a base64 auth entry) or set `FORGER_OCI_BEARER=<token>` for a pre-resolved bearer token.

If a downstream wants ghcr.io or a self-hosted Harbor:
```kdl
builder { binary "oci://ghcr.io/myorg/forger:linux-gnu" sha256="..." }
```

The OCI source kind ships in this feature, not later — user explicitly asked
for the full feature.

### OCI layer model

forger expects an OCI ARTIFACT, not a container image. Acceptable layer media types are `application/vnd.refraction.forger.binary.v1` (preferred) and `application/octet-stream` (fallback for `oras push --artifact-type` users). Layers using `application/vnd.oci.image.layer.v1.tar`, `.tar+gzip`, or any `application/vnd.docker.image.rootfs.*` type are rejected with `OciUnsupportedLayer { media_type }` — the error message must instruct the publisher to use `oras push <ref> ./forger:application/vnd.refraction.forger.binary.v1`. References that resolve to an OCI ImageIndex (multi-platform manifest list) are rejected with `OciMalformed` and a suggestion to template `{triple}` into a per-platform tag; multi-platform index resolution is an explicit follow-up (already listed out-of-scope).

## Files to touch

- `crates/spec-parser/src/schema.rs` — `BuilderBinary` struct, `BuilderNode.binary`
- `crates/spec-parser/src/resolve.rs` — pass-through layering for `binary`
- `crates/spec-parser/src/lib.rs` — tests for parsing the new block
- `crates/forge-builder/src/binary.rs` — `BinarySource` enum, parse/resolve/fetch/verify, cache strategy
- `crates/forge-builder/src/error.rs` — new error variants: `BinarySourceInvalid`, `BinarySha256Mismatch`, `OciPullFailed`
- `crates/forge-builder/src/lib.rs` — accept `builder_binary` parameter, plumb to resolver
- `crates/forger/src/main.rs` — `--builder-binary` and `--builder-binary-sha256` flags on `build`
- `crates/forger/src/commands/build.rs` — thread the flags through
- `crates/forge-builder/Cargo.toml` — add `oci-client`, `sha2`, `hex` (if not already pulled in transitively)
- `book/src/spec/builder.md` (extend with `binary` child documentation), `book/src/reference/cli.md` (document `--builder-binary`, `--builder-binary-sha256`, `--output-name`), `book/src/spec/builder-binary.md` (new long-form treatment), and `book/src/SUMMARY.md` (MANDATORY — add the new page under `# The KDL Spec Language` after `Builder Configuration`; mdbook does not auto-discover files and an orphaned page is invisible in the sidebar).
- `images/*.kdl` example snippets — *optional*, only if the repo has spec examples that benefit from showing the new field

## Implementation order (one PR, multiple commits)

1. **Schema** — `BuilderBinary` struct, field on `BuilderNode`, KDL parse tests.
2. **Source enum** — `BinarySource` + `from_str` with prefix detection and the spec-dir base for relative paths; unit tests covering valid and rejected inputs.
3. **Resolver core** — refactor `resolve_forger_binary` to take `Option<&BinarySource>`, implement the priority chain. Add unit tests with a small temp-dir cache.
4. **HTTP fetch + sha256** — implement URL kind end-to-end with content-addressed cache, atomic rename, hash verification. Integration test with `wiremock`.
5. **OCI fetch** — implement OCI kind; integration test with `wiremock` (registry v2 mock) or `testcontainers-rs` if the latter is already a dev-dep.
6. **CLI flags** — wire `--builder-binary` / `--builder-binary-sha256` through the `build` command (mirroring `--builder-image`).
7. **Env vars** — read `FORGER_BUILDER_BINARY` / `_SHA256` in the resolver entry point.
8. **Docs** — mdbook page; CLI `--help` strings.
9. **Output naming knob** *(small, related)* — add `--output-name <name>` to `forger build` that overrides `{target.name}.qcow2`. Default behavior unchanged. Threads through `phase2/mod.rs::55`, `phase2/qcow2_ext4.rs::41-42`, `phase2/qcow2_zfs.rs::53-54`, `phase2/oci.rs::50`, `phase2/artifact.rs::18`, `forge-builder/src/push.rs::20`. This lets consumers like solstice-ci get `solstice-ubuntu-22.04.qcow2` without renaming the KDL target. (Inclusion rationale: same caller, same surface area, ships in the same release; splitting is overhead.) `--output-name` REQUIRES `--target` to be set (single-target build). Using it without `--target` is a usage error with message `--output-name requires --target to disambiguate which artifact to rename`. Enforce via clap-level `requires = "target"`. It is valid to pass `--output-name` even when the value matches the resolved target name (the flag is an unconditional override, not a mismatch assertion).

## Errors (`forge-builder/src/error.rs` additions)

```rust
#[error("invalid builder binary source {source:?}: {detail}")]
#[diagnostic(code(forge_builder::binary_source_invalid))]
BinarySourceInvalid { source: String, detail: String },

#[error("sha256 mismatch for builder binary from {url}: expected {expected}, got {got}")]
#[diagnostic(code(forge_builder::binary_sha256_mismatch))]
BinarySha256Mismatch { url: String, expected: String, got: String },

#[error("OCI pull failed for {reference}: {detail}")]
#[diagnostic(code(forge_builder::oci_pull_failed))]
OciPullFailed { reference: String, detail: String },

#[error("OCI image {reference} must have exactly one layer, found {layer_count}")]
#[diagnostic(code(forge_builder::oci_malformed))]
OciMalformed { reference: String, layer_count: usize },

#[error("OCI artifact {reference} has unsupported layer media type {media_type}")]
#[diagnostic(code(forge_builder::oci_unsupported_layer))]
OciUnsupportedLayer { reference: String, media_type: String },
```

## Testing

- **Unit (spec-parser)**: parse a KDL with `builder { binary "https://..." sha256="..." }` and assert the struct round-trips. Parse a spec WITHOUT a `binary` block and assert backwards compat.
- **Unit (forge-builder)**: `BinarySource::from_str` happy paths (path, file://-not-needed, http, https, oci) and error paths (unprefixed string, empty string, oci:// with bad ref). `{triple}` substitution.
- **Integration (HTTP)**: `wiremock` serves a 1MB pseudo-binary; resolver fetches, verifies sha256, returns a cache path that points at the right bytes; second call hits cache without HTTP.
- **Integration (HTTP sha256 mismatch)**: server returns bytes with wrong sha256 → resolver returns `BinarySha256Mismatch`, cache is NOT populated.
- **Integration (OCI)**: serve a v2 registry mock with a single-layer image; resolver pulls successfully; multi-layer image returns `OciMalformed`.
- **Integration (priority)**: CLI flag overrides spec block; spec block overrides env; env overrides dev fallback; dev fallback overrides release URL.
- **Integration (relative path)**: `binary "./bin/forger"` in a spec at `/tmp/foo/img.kdl` resolves to `/tmp/foo/bin/forger`.
- **No new public-API breakage**: existing tests stay green.
- **Integration (no fall-through on configured-source failure)**: configure `--builder-binary` at a URL that returns 404 → resolver returns `BinaryDownloadFailed` (or equivalent); dev fallback is NOT stat-ed and the release-URL host receives no request (assert via wiremock fail-on-hit on the release host).
- **Integration (cache poisoning)**: pre-populate the content-addressed cache entry with wrong bytes, configure the matching sha256, invoke resolver → cache entry is deleted and re-fetched; returned bytes match the expected hash.
- **Integration (path source copy semantics)**: configure `binary "./tmp/forger"`, resolve, delete the source file, resolve again → cache hit returns valid bytes (proves copy, not symlink).
- **Integration ({triple} fetch)**: wiremock asserts the request path contains the resolved triple (e.g. `x86_64-unknown-linux-gnu`), not the literal `{triple}` token. Cover URL and OCI source kinds.
- **Integration (OCI sha256 mismatch)**: registry mock serves a single-layer artifact whose payload hashes differently from the configured sha256 → resolver returns `BinarySha256Mismatch`; cache is not populated.
- **Integration (OCI unsupported layer)**: registry mock serves a `tar+gzip` layer → resolver returns `OciUnsupportedLayer` with a helpful publisher hint.
- **Integration (env-var source)**: with no CLI flag and no spec block, set `FORGER_BUILDER_BINARY=<url>` and `FORGER_BUILDER_BINARY_SHA256=<hex>`; resolver fetches and verifies as if specified in the spec, including `{triple}` substitution.
- **Unit (cache key derivation)**: for the same source string with no sha256 the resolver produces the same cache filename across runs; different source strings (and different triples) produce different filenames; sha1 is computed over the POST-substitution string.
- **Unit (output-name across formats)**: for each output format (qcow2-ext4, qcow2-zfs, oci, artifact tar), verify `--output-name <n>` produces `<n>.{ext}` and that omitting it preserves `<target>.{ext}`. Also assert that omitting `--target` while passing `--output-name` is a clap-level usage error.
- **Unit (spec-parser, {triple} preservation)**: parse `binary "http://host/forger-{triple}" sha256="abc"` and assert `source` round-trips with the literal `{triple}` token intact (no KDL-level interpolation).

## Backward compatibility

- KDL specs that omit `builder.binary` parse identically.
- The existing dev-mode lookup runs unchanged when no source is configured.
- The release-URL fallback stays in place (last-resort), so anyone who *does*
  publish a v0.1.0 asset later still benefits from it.

## Open questions resolved

- *"Should sha256 be required?"* — no, but warn when omitted.
- *"Should OCI use the layer digest as the verifier?"* — The user-supplied sha256 is matched against the bytes written to the cache (the binary). Under the artifact contract, that equals the manifest layer digest.
- *"Should the resolver fall through on a 404 of a user-configured URL?"* —
  no; explicit configuration is an explicit intent.
- *"Per-source-kind cache namespaces?"* — no; one flat cache, file names
  encode the discriminator (sha256 or source hash).

## Out-of-scope follow-ups (not blocking this feature)

- Bearer-token / mTLS auth for HTTP/OCI sources beyond `Authorization` env
  var passthrough.
- Multi-arch OCI manifest selection (`linux/amd64` vs `linux/arm64`).
- Auto-detection of a workspace native build to seed an HTTP server.
- OCI credential-helper invocation (credsStore / credHelpers).

## Acceptance criteria

- A clean `cargo build` and `cargo test --workspace` pass on main.
- `forger build --help` shows `--builder-binary` and `--builder-binary-sha256`.
- A KDL spec with `builder { binary "..." sha256="..." }` parses, and the
  resolver fetches/verifies/caches as specified.
- A spec without the field continues to work in dev mode and falls back to
  release URL otherwise.
- `forger build --output-name solstice-ubuntu-22.04 ...` produces
  `solstice-ubuntu-22.04.qcow2` instead of `<target>.qcow2`.
- mdbook page is added to SUMMARY.md under the appropriate section and renders in the sidebar; CLI help reflects `--builder-binary`, `--builder-binary-sha256`, and `--output-name`.
- One PR-equivalent commit series on main, scoped per implementation step.
