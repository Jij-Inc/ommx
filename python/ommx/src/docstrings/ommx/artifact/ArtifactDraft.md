Build an Artifact by adding typed data and metadata, then commit it locally.

A draft collects the layers and annotations that will become an
{class}`~ommx.artifact.Artifact`. The Artifact class contains a complete
executable example that creates a problem and candidate, adds JSON and a table,
exports an archive, and reads it in a separate process with an empty registry.

# Choose a name

| Constructor | Use |
| --- | --- |
| {meth}`new` | Supply an explicit OCI image reference |
| {meth}`new_anonymous` | Generate a local name when you do not need a distribution name |
| {meth}`for_github` | Build a GitHub Container Registry reference and record its source repository |

Every constructor uses the Local Registry. An anonymous name is still a
persistent registry entry, not a temporary file. Its timestamp records the
sender's local clock without a timezone marker; do not use that part of the name
as an absolute timestamp across machines. {meth}`new_anonymous` describes the
complete naming convention.

# Add data and record its meaning

Use {meth}`add_instance`, {meth}`add_parametric_instance`, {meth}`add_solution`,
or {meth}`add_sample_set` for OMMX objects. Their payload formats and OMMX
annotations are handled by the SDK. {meth}`add_json`, {meth}`add_dataframe`, and
{meth}`add_ndarray` store JSON, pandas tables, and NumPy arrays. Their annotation
keywords use `org.ommx.user.` by default; `title="analysis"` becomes the
`org.ommx.user.title` annotation. {meth}`add_annotation` adds manifest-level
metadata rather than a layer annotation.

Adding data returns a {class}`~ommx.artifact.Descriptor`. Retain its digest when
you need to identify that exact layer in a separate note or application record.
Adding several problems and results does not establish their relationships
automatically. {meth}`add_layer` is the low-level operation for data with an
explicit media type and byte representation.

# Commit, export, and distribute

{meth}`commit` finalizes the layer list and creates a registry-resident Artifact.
The draft is consumed by this operation; use a new draft for different content.
Layer data can already have been written locally while building the draft, so
an uncommitted draft does not mean that no disk writes occurred.

Committing does not write a `.ommx` archive or contact a remote registry. Call
{meth}`~ommx.artifact.Artifact.save` on the returned Artifact to export a file, or
{meth}`~ommx.artifact.Artifact.push` to distribute it. A receiver imports an
archive with {meth}`~ommx.artifact.Artifact.import_archive` before reading its
layers. The complete exchange example in {class}`~ommx.artifact.Artifact`
demonstrates these boundaries without a solver or remote service.
