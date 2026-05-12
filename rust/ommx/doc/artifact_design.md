# OMMX Artifact: OCI Layer and Local Registry

## What is an OMMX Artifact?

An **Artifact** is a named, immutable bundle of optimization data —
typically one or more of: `Instance` (problem definition),
`Solution` / `SampleSet` (results), `ParametricInstance`, `ndarray` /
`DataFrame` payloads, and JSON / generic blobs. Each Artifact is
identified by an image reference like
`ghcr.io/myorg/myproblem:v1` or `name@sha256:<digest>`.

OMMX builds Artifact on top of the [OCI Image / Distribution
specifications](https://github.com/opencontainers/) — the same
standards container ecosystems use. Reusing OCI means an Artifact can
be hosted on any OCI-compatible registry (GHCR, ECR, GAR, Docker Hub,
self-hosted `distribution/distribution`, …) without OMMX needing its
own server-side infrastructure, and the same content can be inspected
with off-the-shelf tools (`oras`, `crane`, `skopeo`).

## Conceptual model

| Concept | What it is | OMMX type |
|---|---|---|
| Image reference | The name an Artifact is known by — `host[:port]/name(:tag\|@digest)` | `ommx::artifact::ImageRef` |
| Manifest | Small JSON describing the Artifact: `artifactType`, `config`, an ordered list of layer descriptors, optional `subject` for lineage | An OCI Image Manifest, stored verbatim |
| Descriptor | `{ mediaType, digest, size, annotations }` — a typed pointer to a content-addressed blob | OCI 1.1 descriptor |
| Layer / blob | The actual payload bytes (a serialized `Instance`, a Parquet `DataFrame`, …). Identified by digest | Stored content-addressed |
| Tag | Mutable alias for a digest (e.g. `:v1`, `:latest`) | Lives in the registry's index |
| Digest | Immutable identifier (`sha256:…`); the primary key for an Artifact version | Content hash of the manifest |

OMMX-specific Artifacts are identified by the manifest's top-level
`artifactType` field set to `application/org.ommx.v1.artifact`. This
is the OCI 1.1 pattern: an Image Manifest with `artifactType` plus an
empty `config` descriptor.

## Where Artifacts live

There are three storage locations, all interoperating:

| Location | Purpose | Implementation |
|---|---|---|
| **Local Registry** | Persistent on-disk store / cache / checkout area on the user's machine | SQLite (`SqliteIndexStore`) + filesystem CAS (`FileBlobStore`) under `$OMMX_LOCAL_REGISTRY_ROOT` or the OS data directory |
| **Remote registry** | Sharing and distribution across machines and teams | Any OCI-compliant HTTP registry. Transport is `oci-client` |
| **`.ommx` archive** | Single-file exchange format. A tar of OCI Image Layout | Native tar reader / writer; produced by `LocalArtifact::save` |

The Local Registry is the centre of gravity for SDK calls: builds
publish into it, loads read from it, pushes upload from it, and saves
export from it. Remote registries and `.ommx` archives are
interchange boundaries that move bytes in and out.

## Typical workflows

```rust,no_run
# use ommx::artifact::{InstanceAnnotations, SolutionAnnotations};
# use ommx::v1;
# fn example() -> ommx::Result<()> {
# let instance = v1::Instance::default();
# let instance_annotations = InstanceAnnotations::default();
# let state = v1::State::default();
# let solution_annotations = SolutionAnnotations::default();
use ommx::artifact::{ImageRef, LocalArtifact, LocalArtifactBuilder};
use ommx::artifact::local_registry::{pull_image, LocalRegistry};
use std::sync::Arc;

let image: ImageRef = "ghcr.io/myorg/myproblem:v1".parse()?;

// 1. Build a new Artifact and publish it to the default Local Registry.
let mut builder = LocalArtifactBuilder::new(image.clone());
builder.add_instance(instance, instance_annotations)?;
builder.add_solution(state, solution_annotations)?;
let artifact = builder.build()?;

// 2. Open an existing Artifact from the Local Registry.
let opened = LocalArtifact::open(image.clone())?;

// 3. Pull from a remote registry into the Local Registry, then open.
let registry = Arc::new(LocalRegistry::open_default()?);
pull_image(&registry, &image)?;
let pulled = LocalArtifact::open(image.clone())?;

// 4. Share the Artifact by pushing it to its remote registry.
artifact.push()?;

// 5. Export to a `.ommx` archive for ad-hoc exchange.
artifact.save(std::path::Path::new("myproblem-v1.ommx"))?;
# Ok(())
# }
```

CLI equivalents — `ommx inspect`, `ommx push`, `ommx pull`,
`ommx save`, `ommx load`, `ommx artifact import` — live in the `ommx`
binary. Building is programmatic (via the SDK). The Python SDK
exposes the same operations through `ommx.v1.Artifact` /
`ommx.v1.ArtifactBuilder`; on the Python side `Artifact.load` combines
the "pull from remote then open" steps shown above.

## What this document covers

The rest of this doc is the **architectural reference** for two
internal layers:

1. **OCI implementation policy** — what OMMX implements itself versus
   what it delegates to external OCI crates, and how the manifest
   format is fixed.
2. **Local Registry model** — the IndexStore + BlobStore split, the
   atomic publish primitive, and how the registry interoperates with
   `.ommx` archives, remote registries, and the v2-era path/tag
   directory cache.

Per-API specifics (function signatures, error types, exact field
names) live in module rustdoc under `ommx::artifact::*` rather than
this doc. For the user-visible API surface and v2 → v3 migration
steps, see [`crate::doc::release_note`] and
[`crate::doc::migration_guide`].

---

## 1. OCI Implementation Policy

### 1.1 External crate scope

OMMX does not absorb a full OCI-stack implementation. The Local Registry
is an IndexStore + BlobStore, not an OCI Image Layout, so a single
"archive / dir / remote" trait abstraction would not fit the central
model.

External OCI crates are scoped to remote registry transport:

| Area | Policy |
|---|---|
| Remote manifest GET / PUT | `oci-client` (ORAS project) |
| Remote blob upload / download | `oci-client` |
| Auth / credential handling | `oci-client` + `docker_credential` |
| Cross-repository mount | `oci-client` if supported |
| Referrers API | `oci-client` if supported |

`oci-client` selection rationale:

- Built on `oci-spec` 0.9, which OMMX already uses. `oci_client::Reference`
  is a re-export of `oci_spec::distribution::Reference`, so the parser
  used by `ImageRef` and the type pushed by the transport are the same
  type.
- Apache-2.0, actively maintained as an ORAS sub-project.

OMMX-owned implementations:

- Artifact manifest / config / layer semantics.
- Descriptor / digest / media-type public API.
- IndexStore / BlobStore.
- Atomic publish and GC.
- `.ommx` archive import / export (native tar reader/writer).
- Explicit OCI directory layout import / export.
- Legacy v2 path/tag OCI dir layout migration.
- Image reference public API (`ommx::artifact::ImageRef`).

### 1.2 Public API surface

Descriptor / digest / media-type / image-reference types are
OMMX-owned. The crate does not re-export third-party OCI types on its
public surface. `oci_spec` types are used internally as serde helpers
and stay confined to internal modules.

`ommx::artifact::ImageRef` is a newtype around
`oci_spec::distribution::Reference`. Display produces the canonical
`host[:port]/name(:tag|@digest)` form. The newtype guarantees a
canonicalisation invariant — see the rustdoc on `ImageRef` for the
specific rules (Docker Hub host normalisation, `library/` prefix
completion).

### 1.3 Registry compatibility

OCI v1.1 `subject` and the Referrers API are not uniformly supported
across registries. v3 takes no implicit fallback:

- Archives and explicitly exported OCI Image Layout directories are
  fully under OMMX control, so `subject` is written as-is.
- For remote registries that reject `subject` push, OMMX surfaces an
  explicit error rather than silently falling back to annotation-based
  encoding. A fallback shape will be designed when a real incompatible
  registry case appears.

### 1.4 OCI Image Layout role

OCI Image Layout (`oci-layout`, `index.json`, `blobs/`) is **not** the
Local Registry's internal format. The BlobStore root does not contain
an `oci-layout` file; the `blobs/<algorithm>/<encoded>` key scheme it
uses is a borrowed CAS naming convention, not OCI Image Layout.

OCI Image Layout is an **interchange format** at boundaries:

- `.ommx` archive import / export.
- Explicit OCI directory layout export.
- Legacy v2 directory layout import.
- Remote OCI registry push / pull.
- Inspection with standard tools (`oras`, `crane`, `skopeo`).

Inside the registry, IndexStore is the source of truth for refs /
manifests / entries, and BlobStore is the source of truth for
content-addressed bytes. When standard OCI Image Layout is required,
both stores are materialised into a directory or archive containing
`oci-layout`, `index.json`, and `blobs/`.

### 1.5 Manifest format

The Local Registry stores manifests as OCI Image Manifest
(`application/vnd.oci.image.manifest.v1+json`) **only**. OMMX-specific
artifacts are identified by the top-level `artifactType` field set to
`application/org.ommx.v1.artifact`, with `application/vnd.oci.empty.v1+json`
as the `config` descriptor — the OCI 1.1 recommended pattern.

The deprecated OCI Artifact Manifest
(`application/vnd.oci.artifact.manifest.v1+json`) is rejected at parse
time, not handled via a second enum variant. Rationale:

- **Spec status:** image-spec 1.1 formally removed it; the Artifact
  Manifest doc is archived. The successor pattern is "Image Manifest +
  `artifactType` + empty config".
- **Registry reality:** `distribution/distribution` v2 (the upstream
  reference registry) rejects Artifact Manifest with `MANIFEST_INVALID`
  under default configuration.
- **Implementation simplicity:** reader / writer / tests collapse to a
  single format.

Parse-time identification checks only `artifactType`. The `config`
blob is unverified — legacy v2 OMMX artifacts that carry an
OMMX-specific config (`application/org.ommx.v1.config+json`) remain
readable without a separate import codepath.

Native build (`LocalArtifactBuilder`) emits a manifest byte-compatible
with v2 archive build (`ocipkg::OciArtifactBuilder::new`), modulo JSON
field ordering. Specifically:

- `schemaVersion: 2`.
- `artifactType: application/org.ommx.v1.artifact`.
- `config`: OCI 1.1 empty descriptor (`application/vnd.oci.empty.v1+json`
  pointing at the 2-byte JSON `{}`).
- `layers[]`: each layer renders an `annotations` field (empty object
  if no annotations).
- Top-level `mediaType` is intentionally omitted; HTTP `Content-Type`
  is supplied by the transport at push time.

v3 sorts JSON fields alphabetically via `stable_json_bytes` for
reproducible digests; v2 used struct declaration order. No format
conversion is provided since both v2 and v3 emit Image Manifest with
the same `artifactType` identity.

External Artifact Manifest input is an explicit error, not a silent
fallback. This is the registry's only ingestion-format restriction.

---

## 2. Local Registry Model

### 2.1 Role

The Local Registry is a persistent store / cache / checkout area for
named artifacts. It is not just a developer cache — the design admits
shared filesystem and cloud-backed blob storage where multiple
processes or runners read and write concurrently.

The registry has two layers:

| Layer | Responsibility |
|---|---|
| IndexStore | Queryable index of image name, tag, digest, manifest, and DataStore entry metadata |
| BlobStore | Digest-addressed byte storage |

### 2.2 IndexStore

IndexStore holds the registry's mutable state. The implementation is
swapped per storage profile:

| Implementation | Use case |
|---|---|
| SQLite (`SqliteIndexStore`) | Single-node local cache, tests, CLI workflows |
| PostgreSQL (planned) | Shared registry, multi-node writers, cloud deployment |

SQLite is the in-tree implementation. It accepts multiple processes on
the same node writing briefly to the same local cache — writes are
serialised by SQLite transactions. It is **not** intended for
high-frequency writers, long-running transactions, or mounted
object-storage-backed multi-writer registries; those use cases require
PostgreSQL or another external transactional database.

Minimum information kept in IndexStore:

| Logical table | Content |
|---|---|
| refs | image name + tag/digest reference → manifest digest |
| manifests | manifest digest → media type, size, subject, annotations, created_at |
| manifest_layers | manifest digest → layer descriptors |
| blobs | content digest → size, media type, storage URI, kind, last-verified time |
| entries | DataStore entry name/type → descriptor, manifest digest, query metadata |

`entries` is a query index, not the source of truth. A full Artifact
can always be reconstructed from a manifest and its referenced blobs.

### 2.3 BlobStore

BlobStore holds content-addressed bytes — layer payloads, config
bytes, manifest JSON, trace layers, and every other content-addressed
object in an Artifact. The filesystem backend (`FileBlobStore`) uses
keys of the form `blobs/<algorithm>/<encoded>`; a GCS backend would
use the same logical key as the object name.

This `blobs/` prefix borrows the digest-addressed naming from OCI
Image Layout but **does not** make the BlobStore root an OCI dir. The
root contains no `oci-layout` or `index.json`.

Rules:

- Writes are digest-addressed and idempotent.
- Writing different bytes under an existing digest is an error.
- Reads may verify size / digest on demand.
- Listing and queries go through IndexStore, never through BlobStore
  enumeration.

### 2.4 Atomic publish

"Publish" is borrowed from OCI Distribution and is used here as a
**registry-side verb**: the registry receives a manifest, registers
the manifest and its content-addressed blobs in IndexStore, and points
the requested `image_name` ref at the new manifest digest, in a single
atomic operation.

The Git analogy: writing an object into `.git/objects/` and advancing
`refs/heads/<branch>`. In the Build / Seal / View phases used by the
SDK, the Seal phase's I/O step calls the publish primitive.

`ArtifactBuilder::build()` is the user-side "commit"; the registry-side
"publish" is `LocalRegistry::publish_artifact_atomic`. The split is
intentional — Git makes the same distinction between `git commit`
(user-visible) and the underlying object / ref updates.

Since DB and BlobStore are not jointly transactional, publish order is
fixed:

1. Build layer / config / manifest bytes in the Build phase.
2. Compute each digest and size.
3. Upload all content-addressed objects to BlobStore (idempotent).
4. In a single IndexStore transaction, insert / update blobs, manifest,
   entries, and ref.
5. Commit the transaction; the artifact becomes visible.

A failure between steps 3 and 4 leaves uncommitted blobs in BlobStore;
those are reclaimed by GC. A failure inside the transaction makes the
artifact invisible.

Tag updates run as ref updates inside the IndexStore transaction. The
v2-era "writer writes directly to the final OCI dir" approach is gone.

For concurrent publishes:

- Writes to unique ref / digest combinations succeed independently.
- For the same mutable ref with different digests:

| Primitive | Behaviour |
|---|---|
| `RefConflictPolicy::KeepExisting` | Keep the existing ref; conflict if a different digest is already published |
| `RefConflictPolicy::Replace` | Update ref to the new digest unconditionally (caller-explicit) |

The four-state result is exposed as
`RefUpdate::{Inserted, Unchanged, Replaced, Conflicted}`.

Higher-level semantics for `latest`, `active`, `current run`
(last-writer-wins vs. CAS vs. promote-only) belong to the Experiment /
Run layer. The Local Registry layer only exposes the atomic primitive.

### 2.5 Read / query API

The v3 Local Registry API is centred on reference / descriptor / blob
reader, not paths:

| API | Behaviour |
|---|---|
| `Artifact::exists(ref)` | Resolve `ref` via IndexStore |
| `Artifact::resolve(ref) -> Descriptor` | Resolve tag or digest reference to a manifest descriptor |
| `Artifact::load(ref)` | Open a read-only view backed by manifest + referenced blobs |
| `Artifact::list(prefix=...)` | IndexStore query — no filesystem or BlobStore scan |
| `Artifact::open_blob(digest)` | Internal blob reader with digest / size verification |

`list` is IndexStore-only by design. A full filesystem scan of the
registry root is not part of normal listing.

**Lazy auto-migration for legacy v2 caches.** v2 OMMX stored each
`(image_name, tag)` as a standalone OCI Image Layout directory under
`<root>/<image_name>/<tag>/`. v3 reads v2 caches transparently:

- The v2 layout is read-only — new writes never land in it.
- Explicit migration is via `import_legacy_local_registry*` / the
  `ommx artifact import` CLI, which validates manifests and blobs and
  registers them in IndexStore + BlobStore while preserving manifest
  bytes and digest identity.
- High-level APIs like `Artifact::load(image)` also probe a single
  legacy path (`Path::exists()` on one directory, not a recursive
  scan) on IndexStore miss, and import that single image transparently
  on first hit. Subsequent calls go through the IndexStore fast path.

The "no recursive legacy scan" invariant is preserved: probes are
single-path existence checks; reads always go through IndexStore.
Listing APIs (`Artifact::list`) remain IndexStore-only and never look
at legacy paths.

**Conflict handling at import.** When the same ref already exists with
a different manifest digest, the default policy keeps the existing ref
and skips the entry. `--replace` (CLI) or `RefConflictPolicy::Replace`
(SDK) is the only way to overwrite. Same-digest re-imports are
idempotent re-verifications, not conflicts.

For concurrent import, ref publishes are atomic inserts. Same
(ref, digest) imported from multiple processes results in one
successful publish and verifies for the rest. Different digests racing
the same ref under default policy: first writer wins, others are
conflict-skipped. `--replace` is explicitly destructive and uses
last-writer-wins. BlobStore writes use a temporary file in the same
directory and atomically rename to the final CAS path.

### 2.6 Import / export

OCI Image Layout compatibility lives at the import / export boundary.

**Import.** Reads external OCI-format content and registers
`manifest bytes / each blob byte-for-byte` into IndexStore + BlobStore.
Accepted manifest format is OCI Image Manifest only (see §1.5).
Identity is preserved — no format conversion, no rewriting of bytes
or digests. Supported sources:

- A single OCI Image Layout directory (`oci-layout` + `index.json` +
  `blobs/`). Same import works for `oras` / `crane` / `skopeo` output
  and for individual v2 OMMX entries.
- A v2 OMMX local registry layout (path/tag tree). The legacy importer
  recurses the root, enumerates OCI dirs, and applies the same
  per-directory import in a batch.
- A `.ommx` OCI archive (tar.gz), via the native tar reader.
- A remote OCI registry. Manifest and blobs are pulled into BlobStore;
  IndexStore transaction registers the ref.

External Artifact Manifest input is an explicit error, not a silent
fallback.

**Default export.** Given a manifest descriptor as root, collect the
manifest's *material closure* and materialise a standard OCI Image
Layout (`oci-layout`, `index.json`, `blobs/`). This is `depth=1` in
the Git analogy.

**History bundle export.** Opt-in. From a manifest, follow the
`subject` chain and materialise the *lineage closure* into the same
archive / directory. Equivalent to `--depth=N` or a full history
bundle. This is for offline `history()` traversal and is intentionally
separate from default export.

**Remote push.** Read manifest + blobs from IndexStore + BlobStore and
send via OCI Distribution API.

Closure definitions:

| Closure | Includes | Used by |
|---|---|---|
| material closure | Root manifest, `config` blob (including empty config), every `layers[]` descriptor, trace layer — everything needed to reconstruct the snapshot | Default export |
| lineage closure | Each parent manifest reachable via `subject`, plus each parent's material closure | History bundle export |

`subject` remains in the manifest as a descriptor under default export
but its target is not part of the material closure. The parent digest
is visible; the parent manifest / blobs may be absent.
