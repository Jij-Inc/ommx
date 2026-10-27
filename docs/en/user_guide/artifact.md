# Artifact storage and identity

**An Artifact packages problems, evaluated candidates, analysis data and other content for storage and exchange.** Evaluating or transforming a problem, or using an Adapter, does not require creating an Artifact.

To create a first exchange file, start with [Exchange a problem and evaluation results](../tutorial/share_in_ommx_artifact.md). This page explains the state, location and identity of stored data.

## Data and its relationships

An Artifact contains payloads and an inventory describing their types and metadata. For example, it can group an `Instance`, a candidate's evaluated `Solution`, and a JSON analysis note. Each payload is a **layer**; the inventory is a **manifest**.

A layer's **Media Type** specifies how to interpret its data. Typed APIs such as `add_instance()` and `get_instance()` save and restore problems without manually choosing an encoding. **Annotations** can describe layers.

Simply storing several problems and results does not automatically establish arbitrary relationships between them. Explicitly retain relationships your application needs, such as which input produced which result. A {class}`~ommx.artifact.Descriptor` contains a layer's Media Type, size and content identifier. The Tutorial also saves those identifiers in its analysis note.

## Drafts and committed Artifacts

Use {class}`~ommx.artifact.ArtifactDraft` to assemble layers and descriptions. `commit()` finalizes the inventory and makes an {class}`~ommx.artifact.Artifact` available from the Local Registry. It does not provide later layer additions to that committed content. Create a new Draft for different content.

`commit()` neither exports a file nor sends data remotely. Call `save()` or `push()` on the committed Artifact when needed. Draft operations can already write local data, so uncommitted does not mean nothing has been written to disk.

## Three storage and exchange locations

| Location | Purpose | Main operations |
|---|---|---|
| Local Registry | Persistent storage and reuse on this machine | `commit()`, `Artifact.load()` |
| `.ommx` file | Exchange through ordinary file transfer | `save()`, `Artifact.import_archive()` |
| Remote registry | Distribution to teams and other machines | `push()`, `Artifact.load()` when a reference is absent locally |

The **Local Registry** is the SDK's local storage. Use its default location, or set `OMMX_LOCAL_REGISTRY_ROOT` at process startup to select another location. Changing it selects the data in that location; it does not move existing files.

`Artifact.import_archive()` imports file contents into the Local Registry. You can later retrieve the imported name with `Artifact.load()`. To inspect an archive's inventory without importing its contents, use {meth}`~ommx.artifact.Artifact.inspect_archive`; this does not restore layer payloads.

A remote registry distributes OCI-format data. Authentication and visibility follow that service's rules. An Artifact is a data package, not an executable container: it does not run an optimization program in Docker. See [Share an Artifact through a registry](../how_to/share_artifact_registry.md).

## Names, tags and content identifiers

A reference such as `ghcr.io/my-team/my-project/candidates:trial-1` includes a location, name and tag. Here `trial-1` is the tag. Names and tags help people find data, but a tag may later point to different content.

A **digest**, by contrast, is computed from content. A manifest digest identifies the Artifact's inventory, and each layer also has its own digest. Record the relevant digest, not only a tag, when you need an exact identity. Manifest and layer digests are not interchangeable. An archive inventory exposes its manifest digest as {attr}`~ommx.artifact.ArchiveManifest.manifest_digest`.

`Artifact.load(name)` checks the Local Registry first, then fetches remotely if absent. Passing an existing tag does not refresh it from the remote registry every time. To keep using a specific distribution, use the digest reference supplied by its publisher.

`ArtifactDraft.new_anonymous()` starts locally without choosing a name. The SDK generates one containing `.ommx.local`; it is not a remote distribution destination. You can still exchange it as a file. If remote distribution is planned, supply a destination name when creating the Draft. Automatic names may be cleanup targets; see [Manage the Local Registry](../how_to/manage_local_registry.md) for retaining data.

## How Experiment differs

Artifact is the general data packaging and storage format. [Experiment](experiment.md) builds on Artifact to organize trials and their conditions/results, including Run and Solve/Sampling relationships. Adding data to a generic Artifact does not turn it into an Experiment.

Neither feature fills in unsaved environment or source information, nor guarantees rerunning a computation from saved evaluations alone. Save the inputs and descriptions needed for your intended readback.

## SDK and format versions

Python SDK `3.0.0b7` is an API version. It differs from the wire schema of OMMX Messages inside layers and from the Artifact format/type. This SDK writes problems and evaluations as `ommx.v2` Messages, while the generic Artifact type is `application/org.ommx.v1.artifact`. These `v1` and `v2` values are not instructions to install that Python SDK major version.

For cross-language reading and writing, use [the Artifact specification matching public b7](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md) and [Message definitions](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto). Formats can change in development versions: use documents matching your target SDK rather than mixing versions. See {mod}`ommx.artifact` for exact Python API conditions.
