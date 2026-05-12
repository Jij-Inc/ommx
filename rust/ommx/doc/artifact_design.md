# OMMX Artifact Specification

This document specifies the OMMX Artifact format — what bytes
constitute an OMMX Artifact and how those bytes are positioned
within the OCI ecosystem. The OMMX Rust SDK builds, stores, and
transports artifacts according to this spec; an alternative reader
or writer (an `oras`-style tool, a registry inspector, a second-party
SDK) needs to match it byte-for-byte to interoperate.

What this document **does not** cover:

- **SDK API specifics** (function signatures, error types, exact field
  names) live in module rustdoc under `ommx::artifact::*`.
- **Local Registry implementation** (SQLite + filesystem CAS, atomic
  publish primitives, lazy auto-migration) lives in the
  `ommx::artifact::local_registry` module rustdoc.
- **User-facing API surface and migration steps** live in
  [`crate::doc::release_note`] and [`crate::doc::migration_guide`].

## What is an OMMX Artifact?

An **Artifact** is a named, immutable bundle of optimization data —
typically one or more of: `Instance` (problem definition),
`Solution` / `SampleSet` (results), `ParametricInstance`, `ndarray` /
`DataFrame` payloads, and JSON / generic blobs. Each Artifact is
identified by an image reference like
`ghcr.io/myorg/myproblem:v1` or `name@sha256:<digest>`.

OMMX rides the [OCI Image / Distribution
specifications](https://github.com/opencontainers/) — the same
standards container ecosystems use. Reusing OCI means an Artifact can
be hosted on any OCI-compatible registry (GHCR, ECR, GAR, Docker Hub,
self-hosted `distribution/distribution`, …) without OMMX needing its
own server-side infrastructure, and the same content can be inspected
with off-the-shelf tools (`oras`, `crane`, `skopeo`).

## Conceptual model

| Concept | What it is |
|---|---|
| Image reference | The name an Artifact is known by — `host[:port]/name(:tag\|@digest)` |
| Manifest | Small JSON describing the Artifact: `artifactType`, `config`, an ordered list of layer descriptors, optional `subject` for lineage. An OCI Image Manifest, stored verbatim |
| Descriptor | `{ mediaType, digest, size, annotations }` — a typed pointer to a content-addressed blob (OCI 1.1) |
| Layer / blob | The actual payload bytes (a serialized `Instance`, a Parquet `DataFrame`, …). Identified by digest |
| Tag | Mutable alias for a digest (e.g. `:v1`, `:latest`) |
| Digest | Immutable identifier (`sha256:…`); the primary key for an Artifact version. Content hash of the manifest |

OMMX-specific Artifacts are identified by the manifest's top-level
`artifactType` field set to `application/org.ommx.v1.artifact`. This
is the OCI 1.1 pattern: an Image Manifest with `artifactType` plus an
empty `config` descriptor.

## Where Artifacts live

There are three storage locations, all interoperating:

| Location | Purpose |
|---|---|
| **Local Registry** | Persistent on-disk store / cache / checkout area on the user's machine. Internal layout is SDK-specific — see the `local_registry` module rustdoc |
| **Remote registry** | Sharing and distribution across machines and teams. Any OCI-compliant HTTP registry, accessed over the OCI Distribution API |
| **`.ommx` archive** | Single-file exchange format. A tar of OCI Image Layout (`oci-layout` + `index.json` + `blobs/`) |

The interchange semantics between these locations are fixed by the
sections below: §1 pins the manifest bytes, §2 pins the OCI Image
Layout boundary that connects the three, §3 pins the behaviour on
remote registry interactions.

---

## 1. Manifest format

The OMMX Artifact manifest is an OCI Image Manifest
(`application/vnd.oci.image.manifest.v1+json`) **only**. The
deprecated OCI Artifact Manifest
(`application/vnd.oci.artifact.manifest.v1+json`) is rejected at
parse time — readers must not accept it. Rationale:

- **Spec status:** image-spec 1.1 formally removed it; the Artifact
  Manifest document is archived. The successor pattern is "Image
  Manifest + `artifactType` + empty config".
- **Registry reality:** `distribution/distribution` v2 (the upstream
  reference registry) rejects Artifact Manifest with
  `MANIFEST_INVALID` under default configuration.

### 1.1 Identification

An OMMX Artifact is identified by the manifest's top-level
`artifactType` field:

```text
artifactType = "application/org.ommx.v1.artifact"
```

This is the only field a reader must check to identify an OMMX
Artifact. The `config` blob is not part of the identification — a
legacy v2 OMMX manifest that carries
`application/org.ommx.v1.config+json` in its `config` descriptor
remains a valid OMMX Artifact under this spec, and readers must not
reject it.

### 1.2 Required fields

```jsonc
{
  "schemaVersion": 2,
  "artifactType": "application/org.ommx.v1.artifact",
  "config": {
    "mediaType": "application/vnd.oci.empty.v1+json",
    "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    "size": 2
  },
  "layers": [
    {
      "mediaType": "...",
      "digest": "sha256:...",
      "size": 1234,
      "annotations": { "...": "..." }
    }
  ],
  "subject": {
    "mediaType": "application/vnd.oci.image.manifest.v1+json",
    "digest": "sha256:...",
    "size": 1234
  }
}
```

Field-by-field:

- `schemaVersion`: integer `2`.
- `artifactType`: `application/org.ommx.v1.artifact`.
- `config`: the OCI 1.1 empty descriptor —
  `mediaType: application/vnd.oci.empty.v1+json`, pointing at the
  2-byte JSON `{}` blob whose digest is
  `sha256:44136fa3…ff8a` and whose size is `2`. The empty config has
  no annotations.
- `layers`: ordered list of layer descriptors. Each descriptor
  renders the `annotations` field, including an empty object `{}` if
  no annotations apply. Layer ordering is preserved across
  build / import / pull / push round-trips and forms part of the
  manifest digest.
- `subject`: optional. If present, points at a parent OMMX manifest
  for lineage. `mediaType` is fixed at
  `application/vnd.oci.image.manifest.v1+json`. Absence means "no
  parent".
- Top-level `mediaType`: intentionally **not emitted**. HTTP
  `Content-Type` is supplied by the transport at push time.

Other top-level fields defined by OCI Image Manifest (`annotations`)
are permitted but not required.

### 1.3 Byte-level reproducibility

The Rust SDK serialises manifests with JSON fields sorted
alphabetically, so the same logical manifest always produces the
same bytes and therefore the same manifest digest. This is a
property of the canonical OMMX writer; readers should not assume
alphabetical ordering on input. Manifests authored by other tools or
by OMMX v2 (which used Rust struct declaration order) are valid as
long as the JSON parses and the identification rule in §1.1 holds.

---

## 2. OCI Image Layout boundary

OCI Image Layout (`oci-layout` marker file + `index.json` + `blobs/`
directory) is **not** the Local Registry's internal format. It is an
**interchange format** used only at boundaries:

| Boundary | Direction | Format |
|---|---|---|
| `.ommx` archive | Import / export | Tar of OCI Image Layout |
| Explicit directory export | Export | OCI Image Layout |
| v2 OMMX local registry tree | Import (legacy) | OCI Image Layout per `(image_name, tag)` |
| Remote OCI registry | Push / pull | OCI Distribution API over HTTP |
| Standard OCI tools (`oras`, `crane`, `skopeo`) | Inspection / interop | OCI Image Layout |

An OMMX Artifact materialised into an OCI Image Layout (whether
inside a `.ommx` archive or as a directory tree) contains:

- An `oci-layout` marker file with version `1.0.0`.
- An `index.json` listing the artifact manifests being exported.
  Each entry carries an `org.opencontainers.image.ref.name`
  annotation giving the OMMX image reference (`host[:port]/name:tag`
  form), so an importer can reconstruct the original name without
  side-channel information.
- A `blobs/<algorithm>/<encoded>` tree containing the manifest JSON
  bytes, the config blob, and every referenced layer blob — each
  keyed by content digest.

Identity is preserved across this boundary: manifest bytes and layer
bytes are written and read verbatim. An OMMX implementation must not
re-canonicalise, re-order, or otherwise rewrite a manifest while
crossing the boundary — doing so would change the digest and break
content addressing.

---

## 3. Registry compatibility

OCI v1.1 `subject` and the Referrers API are not uniformly supported
across registries. OMMX takes no implicit fallback:

- Archives and explicitly exported OCI Image Layout directories are
  fully under OMMX's control, so `subject` is written into the
  manifest verbatim. Lineage traversal over an exported tree always
  works.
- For remote registries that reject a `subject`-bearing push, OMMX
  surfaces an explicit error rather than silently falling back to
  annotation-based encoding. A fallback shape will be designed when
  a real incompatible-registry case appears.

Old tooling that does not read the `artifactType` field will display
an OMMX Artifact as a generic OCI Image Manifest. That is acceptable
under this spec — OMMX identification is by `artifactType`, and
tools that cannot recognise it simply do not gain OMMX-specific
rendering. Manifest bytes are still valid OCI and still round-trip
through any spec-compliant pipeline.
