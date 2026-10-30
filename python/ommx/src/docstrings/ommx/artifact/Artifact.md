Read a committed package of optimization data from the Local Registry.

An Artifact groups problems, evaluated candidates, tables, and other data with
metadata. It is optional: constructing, transforming, evaluating, or solving an
{class}`~ommx.Instance` does not require an Artifact. Use
{class}`~ommx.experiment.Experiment` when you also want to organize recorded
conditions and results into trials.

# Layers, descriptors, and relationships

Each data item is a **layer**. A **manifest** lists the layers and their metadata.
A layer's {class}`~ommx.artifact.Descriptor` identifies its media type, byte size,
content digest, and annotations. Typed methods such as
{meth}`~ommx.artifact.ArtifactDraft.add_instance` and {meth}`get_instance` choose
and interpret the corresponding serialization format.

The Artifact does not infer which problem produced a particular result merely
because both are present. Record the relationships your application needs. For
example, the complete exchange below records the problem and result layer
digests in a JSON analysis note. When several layers have the same type, select
their descriptors with {meth}`get_layer_descriptor` and pass them to the typed
getters. The convenience properties {attr}`instance` and {attr}`solution` return
the first layer of the requested type.

Annotations describe a layer without decoding its data. For JSON, tables, and
arrays, annotation keywords such as `title="analysis"` are stored under the
default `org.ommx.user.` namespace. They are labels, not uniqueness constraints;
the example deliberately assigns distinct titles.

# Save a problem and candidate, then read them in another process

This example needs only the Python SDK and its pandas dependency. It evaluates
an explicit candidate; it does not run a solver. The sender writes a problem,
its evaluated candidate, an analysis note, and a table into one `.ommx` file.
Build a mutable {class}`~ommx.artifact.ArtifactDraft`, add the data, and commit
its layer list into the Local Registry. Then save the committed Artifact as an
archive; committing locally and exporting a transferable file are separate steps.

```pycon
>>> sender = """
... import sys
... import pandas as pd
... from ommx import Instance
... from ommx.artifact import ArtifactDraft
...
... instance = Instance.maximize()
... x = instance.new_binary("x")
... y = instance.new_binary("y")
... instance.objective = 3 * x + 2 * y
... instance.add_constraint(x + y <= 1)
... solution = instance.evaluate({x.id: 1, y.id: 0})
... assert solution.feasible and solution.objective == 3
...
... draft = ArtifactDraft.new_anonymous()
... instance_layer = draft.add_instance(instance)
... solution_layer = draft.add_solution(solution)
... draft.add_json({
...     "instance_digest": instance_layer.digest,
...     "solution_digest": solution_layer.digest,
...     "note": "An evaluated candidate, not an optimality certificate.",
... }, title="analysis")
... draft.add_dataframe(pd.DataFrame({
...     "item": ["x", "y"], "selected": [True, False],
... }), title="selection")
... artifact = draft.commit()
... artifact.save(sys.argv[1])
... """

```

The receiver uses only the transferred file. It retrieves the problem and
result identified by the analysis note, reevaluates the saved state, and reads
the table. {meth}`import_archive` imports the contents into the receiver's Local
Registry; it does not keep reading the sender's registry or Python objects.

```pycon
>>> receiver = """
... import sys
... from ommx.artifact import Artifact
...
... received = Artifact.import_archive(sys.argv[1])
... by_title = {
...     layer.annotations["org.ommx.user.title"]: layer
...     for layer in received.layers
...     if "org.ommx.user.title" in layer.annotations
... }
... analysis = received.get_json(by_title["analysis"])
... instance = received.get_instance(
...     received.get_layer_descriptor(analysis["instance_digest"])
... )
... solution = received.get_solution(
...     received.get_layer_descriptor(analysis["solution_digest"])
... )
... checked = instance.evaluate(solution.state)
... assert checked.feasible == solution.feasible
... assert checked.objective == solution.objective
... selection = received.get_dataframe(by_title["selection"])
... print(solution.feasible, solution.objective)
... print(selection.loc[selection["selected"], "item"].tolist())
... """

```

Run these two programs in separate processes using the same SDK environment.
The following executable harness creates two initially empty registries and a
temporary shared file. It sets `OMMX_LOCAL_REGISTRY_ROOT` before each child
process starts, so it does not write to your normal registry. A real recipient
can run the receiver program on the transferred archive in their own environment.

```pycon
>>> import os
>>> import subprocess
>>> import sys
>>> from pathlib import Path
>>> from tempfile import TemporaryDirectory
>>> with TemporaryDirectory() as directory:
...     root = Path(directory)
...     sender_registry = root / "sender-registry"
...     receiver_registry = root / "receiver-registry"
...     sender_registry.mkdir()
...     receiver_registry.mkdir()
...     archive = root / "candidate.ommx"
...     sent = subprocess.run(
...         [sys.executable, "-c", sender, str(archive)],
...         env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(sender_registry)},
...         capture_output=True,
...         text=True,
...     )
...     assert sent.returncode == 0, sent.stderr
...     assert not list(receiver_registry.iterdir())
...     received = subprocess.run(
...         [sys.executable, "-c", receiver, str(archive)],
...         env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(receiver_registry)},
...         capture_output=True,
...         text=True,
...     )
...     assert received.returncode == 0, received.stderr
...     print(received.stdout, end="")
True 3.0
['x']

```

The sender's process has exited before the receiver starts, and the receiver's
registry begins empty. The archive is sufficient to recover these stored data.
This does not prove optimality or reproduce the original computation: source
code, solver settings, and environment details are present only if you recorded
them explicitly.

# Local storage, archives, and remote registries

| Destination | Operation | Meaning |
| --- | --- | --- |
| Local Registry | {meth}`~ommx.artifact.ArtifactDraft.commit`, {meth}`load` | Store or reopen a named Artifact on this machine |
| `.ommx` archive | {meth}`save`, {meth}`import_archive` | Export a file for transfer, or import its contents into the receiver's registry |
| Remote OCI registry | {meth}`push`, {meth}`load` | Distribute an Artifact, or retrieve a missing or incomplete local copy |

The **Local Registry** is the SDK's persistent local storage. Choose a separate
root with `OMMX_LOCAL_REGISTRY_ROOT` when starting the process. Changing the root
changes which stored data are visible; it does not move existing data.
{meth}`inspect_archive` reads only the manifest and descriptors without importing
the archive. Its {class}`~ommx.artifact.ArchiveManifest` does not provide decoded
layer contents.

An OCI registry distributes data using container-registry protocols. An OMMX
Artifact is a data package, not an executable container. Remote authentication
and access control belong to the registry service. A local commit neither
exports a file nor uploads data; those require `save()` or `push()` respectively.

# Names, tags, and content digests

A reference such as `ghcr.io/my-team/my-project/candidates:trial-1` identifies a
registry, repository, and tag. A tag is a name that can later refer to different
content. A **digest** identifies content: a manifest digest identifies the
manifest, while each layer has its own digest. The two identify different data.
An archive's manifest digest is available from
{attr}`~ommx.artifact.ArchiveManifest.manifest_digest`.

{meth}`load` uses a complete local copy without contacting the remote registry.
If the reference or any required manifest, config, or layer data is missing, it
attempts a fresh remote pull. A tag with a complete local copy is not refreshed
from the remote server. Use the distributor's digest reference when you need a
specific published Artifact.

{meth}`~ommx.artifact.ArtifactDraft.new_anonymous` generates a local name
containing `.ommx.local`; it is suitable for starting without a distribution
name and for exporting archives, but is not a remote publishing destination.
Use {meth}`~ommx.artifact.ArtifactDraft.new` or
{meth}`~ommx.artifact.ArtifactDraft.for_github` when choosing a remote name.
Anonymous entries remain in local storage until managed explicitly; see
{func}`~ommx.artifact.prune_anonymous` and {func}`~ommx.artifact.gc` for cleanup.

# Data format and SDK versions

The Python SDK version, the Message wire-schema version, and the Artifact type
are separate. For example, `v1` in `application/org.ommx.v1.artifact` does not mean
Python SDK version 1. Typed layer methods handle the payload formats supported by
the installed SDK.

For another language or tool, consult the
[Artifact specification](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md)
and [Message schemas](https://github.com/Jij-Inc/ommx/tree/main/proto).
{meth}`~ommx.Instance.save_mps` instead exports a supported mathematical model
for MPS-compatible tools; it does not preserve all OMMX labels, evaluation
metadata, and recorded data as an Artifact does.
