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

The first source that *resolves to bytes on disk* wins. A configured source
that fails to download is an error, not a fall-through — we don't silently
fall back when the user explicitly asked for something.

### Source kinds

```rust
pub enum BinarySource {
    /// Local file. Copied (not symlinked) into the cache.
    Path(PathBuf),

    /// http://... or https://...
    /// May contain {triple} template token.
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
  - The existing `forger-{triple}-v{version}` layout for the release fallback is unchanged.
- On every URL/OCI fetch: stream to a `.tmp` sibling, hash, verify against
  the configured sha256 (if any), then `rename(2)` atomically. No partial
  files in the cache.
- Local-path source: copy to cache (don't symlink — cache must survive
  source deletion) using the same naming rule.
- Always chmod 0755 on the cache entry.

### sha256 verification

- When sha256 is configured AND the cache entry already exists: re-hash on
  load. Mismatch ⇒ delete and re-fetch. (Defends against cache corruption
  and tools that overwrite cache paths.)
- When sha256 is configured AND the fetched bytes don't match: discard the
  download, surface a `BinarySha256Mismatch { expected, got, url }` error.
- When sha256 is omitted: log a warning at INFO level on every fetch
  (`forger binary fetched without sha256 pin; consider adding one`).

### CLI flag

Add `--builder-binary <source>` to the `build` and `push` subcommands in
`crates/forger/src/main.rs` (alongside `--builder-image`). Threading:

`main.rs::Build::builder_binary` (`Option<String>`)
  → `commands::build::build_cmd(..., builder_binary: Option<&str>)`
  → `forge_builder::lib::build_in_builder(..., builder_binary: Option<&str>)`
  → `binary::resolve_forger_binary(distro, source: Option<&BinarySource>)`

For sha256 on the CLI form: support `--builder-binary <source>` with
`--builder-binary-sha256 <hex>` as a separate flag. (Avoid encoding sha256
inline in the URL — keeps the URL readable and the flag composable.)

### Env var

`FORGER_BUILDER_BINARY=<source>` and `FORGER_BUILDER_BINARY_SHA256=<hex>`,
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

Use the `oci-distribution` crate (already in the broader ecosystem; if not
yet a dependency, add it). Behavior:

- Parse `oci://registry/repo:tag` or `oci://registry/repo@sha256:...`.
- Pull manifest; require exactly one layer (otherwise reject as
  malformed-for-forger with a clear message).
- Stream the layer to the cache `.tmp` path, hash, verify against the
  user-provided `sha256` (NOT the layer digest from the manifest — different
  semantics; layer digest is gzip'd, user's sha256 is uncompressed binary).
- Anonymous pull by default. Honor `DOCKER_CONFIG` (`~/.docker/config.json`)
  for registry credentials so `docker login` works transparently. Token
  exchange follows the standard OCI distribution spec flow.

If a downstream wants ghcr.io or a self-hosted Harbor:
```kdl
builder { binary "oci://ghcr.io/myorg/forger:linux-gnu" sha256="..." }
```

The OCI source kind ships in this feature, not later — user explicitly asked
for the full feature.

## Files to touch

- `crates/spec-parser/src/schema.rs` — `BuilderBinary` struct, `BuilderNode.binary`
- `crates/spec-parser/src/resolve.rs` — pass-through layering for `binary`
- `crates/spec-parser/src/lib.rs` — tests for parsing the new block
- `crates/forge-builder/src/binary.rs` — `BinarySource` enum, parse/resolve/fetch/verify, cache strategy
- `crates/forge-builder/src/error.rs` — new error variants: `BinarySourceInvalid`, `BinarySha256Mismatch`, `OciPullFailed`
- `crates/forge-builder/src/lib.rs` — accept `builder_binary` parameter, plumb to resolver
- `crates/forger/src/main.rs` — `--builder-binary` and `--builder-binary-sha256` flags on `build` and `push`
- `crates/forger/src/commands/build.rs` — thread the flags through
- `crates/forger/src/commands/push.rs` — same
- `crates/forge-builder/Cargo.toml` — add `oci-distribution`, `sha2`, `hex` (if not already pulled in transitively)
- `book/src/SUMMARY.md` + `book/src/builder-binary.md` (new) — operator docs
- `images/*.kdl` example snippets — *optional*, only if the repo has spec examples that benefit from showing the new field

## Implementation order (one PR, multiple commits)

1. **Schema** — `BuilderBinary` struct, field on `BuilderNode`, KDL parse tests.
2. **Source enum** — `BinarySource` + `from_str` with prefix detection and the spec-dir base for relative paths; unit tests covering valid and rejected inputs.
3. **Resolver core** — refactor `resolve_forger_binary` to take `Option<&BinarySource>`, implement the priority chain. Add unit tests with a small temp-dir cache.
4. **HTTP fetch + sha256** — implement URL kind end-to-end with content-addressed cache, atomic rename, hash verification. Integration test with `wiremock`.
5. **OCI fetch** — implement OCI kind; integration test with `wiremock` (registry v2 mock) or `testcontainers-rs` if the latter is already a dev-dep.
6. **CLI flags** — wire `--builder-binary` / `--builder-binary-sha256` through `build` and `push` commands.
7. **Env vars** — read `FORGER_BUILDER_BINARY` / `_SHA256` in the resolver entry point.
8. **Docs** — mdbook page; CLI `--help` strings.
9. **Output naming knob** *(small, related)* — add `--output-name <name>` to `forger build` that overrides `{target.name}.qcow2`. Default behavior unchanged. Threads through `phase2/mod.rs::55`, `phase2/qcow2_ext4.rs::41-42`, `phase2/qcow2_zfs.rs::53-54`, `phase2/oci.rs::50`, `phase2/artifact.rs::18`, `forge-builder/src/push.rs::20`. This lets consumers like solstice-ci get `solstice-ubuntu-22.04.qcow2` without renaming the KDL target. (Inclusion rationale: same caller, same surface area, ships in the same release; splitting is overhead.)

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

## Backward compatibility

- KDL specs that omit `builder.binary` parse identically.
- The existing dev-mode lookup runs unchanged when no source is configured.
- The release-URL fallback stays in place (last-resort), so anyone who *does*
  publish a v0.1.0 asset later still benefits from it.

## Open questions resolved

- *"Should sha256 be required?"* — no, but warn when omitted.
- *"Should OCI use the layer digest as the verifier?"* — no; the user's
  sha256 is over the uncompressed binary they care about, which decouples
  the spec from registry storage format choices.
- *"Should the resolver fall through on a 404 of a user-configured URL?"* —
  no; explicit configuration is an explicit intent.
- *"Per-source-kind cache namespaces?"* — no; one flat cache, file names
  encode the discriminator (sha256 or source hash).

## Out-of-scope follow-ups (not blocking this feature)

- Bearer-token / mTLS auth for HTTP/OCI sources beyond `Authorization` env
  var passthrough.
- Multi-arch OCI manifest selection (`linux/amd64` vs `linux/arm64`).
- Auto-detection of a workspace native build to seed an HTTP server.

## Acceptance criteria

- A clean `cargo build` and `cargo test --workspace` pass on main.
- `forger build --help` shows `--builder-binary` and `--builder-binary-sha256`.
- A KDL spec with `builder { binary "..." sha256="..." }` parses, and the
  resolver fetches/verifies/caches as specified.
- A spec without the field continues to work in dev mode and falls back to
  release URL otherwise.
- `forger build --output-name solstice-ubuntu-22.04 ...` produces
  `solstice-ubuntu-22.04.qcow2` instead of `<target>.qcow2`.
- mdbook page exists, CLI help reflects the flag.
- One PR-equivalent commit series on main, scoped per implementation step.
