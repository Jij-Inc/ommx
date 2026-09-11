# Dataset distributions

MIPLIB 2017 is packaged from the official
[collection archive](https://miplib.zib.de/download.html) using the OMMX MPS
parser. The generator preserves the source authors, licenses, and MIPLIB
annotations on each Instance layer.

## Distribution versioning

The MIPLIB distribution adopted by SDK 2.7 is:

```text
ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}
```

Distribution repositories use
`ghcr.io/jij-inc/ommx/v{major}.{minor}/{dataset}:{instance-name}`.
Changes to the distribution format or the mathematical model require an SDK
minor or major release and a new distribution namespace. An SDK patch release
keeps its adopted distribution version. Do not derive this reference from the
SDK package version at runtime.

A published path/tag reference must never be overwritten. Keep the previous
unversioned MIPLIB repository available for existing SDKs and reproducibility.
The SDK's Rust loader, Python loader, and generator must adopt the same
distribution repository together.

SDK 2.8 adopts `ghcr.io/jij-inc/ommx/v2.8/qplib:{numeric-tag}` for QPLIB,
regenerated with the corrected quadratic coefficient scaling. MIPLIB retains
its v2.7 distribution. Keep both unversioned repositories available for older
SDKs and reproducibility.

The [v2.7 publication record](distributions/v2.7/README.md) includes source
provenance, the full instance inventory, unsupported inputs, and model digests.

## Generate MIPLIB Artifacts

Run from the repository root, with a fresh local registry dedicated to the new
distribution:

```sh
OMMX_LOCAL_REGISTRY_ROOT=/path/to/miplib-v2.7-registry \
  cargo run --locked --release -p dataset -- miplib2017 \
  /path/to/collection.zip --report /path/to/miplib-v2.7-report.csv
```

The CSV contains one row per MPS archive member, with the target reference,
packaging status, variable kinds, constraint count, Instance layer digest, and
failure details. Parsing failures and variable-count mismatches are reported
without creating an Artifact. The command processes the remaining members;
inspect the report even when the command exits successfully. Existing output
directories and storage errors are also reported as failures, rather than
silently counted as successful conversions.

The v2.7 distribution uses the current v2 parser. Unsupported MPS constructs
remain failures; they must not be replaced with relaxed or incomplete models.
Record the archive checksum and retain the report with the publication record.

## Generate QPLIB Artifacts

Download the official [QPLIB archive](https://qplib.zib.de/qplib.zip) and run
from the repository root:

```sh
OMMX_LOCAL_REGISTRY_ROOT=/path/to/qplib-v2.8-registry \
  cargo run --locked --release -p dataset -- qplib \
  /path/to/qplib.zip --report /path/to/qplib-v2.8-report.csv
```

The report covers every `.qplib` archive member, including parse failures,
missing metadata, variable-count mismatches, and storage failures. Valid inputs
retain their authors, license, and QPLIB annotations; the Instance counts
describe the parsed OMMX model. The generator applies the quadratic scaling
correction without adding parser syntax support. Record the archive checksum
and verify objective and constraint values against source models before
publishing the successful rows.

## Publish

The generator only writes local Artifacts. Before publishing, reconcile the
report against the archive and MIPLIB metadata, inspect failures, and check the
affected binary domains. Upload the successful Artifacts with `ommx push`,
using the same dedicated local registry.

For each target reference, first check the remote registry. If it already
exists, do not push it again. Compare its parsed manifest with the local
Artifact, including config and layer descriptors and annotations: matching
content is already published, and any difference must stop publication.
For a reference that does not exist remotely, run:

```sh
OMMX_LOCAL_REGISTRY_ROOT=/path/to/miplib-v2.7-registry \
  ommx push "ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}"
```

After upload, compare the parsed remote and local manifests, check the Instance
layer digest against the generation report, and record the remote manifest
digest in the publication inventory. Verify anonymous read access to the public
package. SDK loaders must not fall back to an older distribution if the adopted
distribution cannot be fetched.

The v2 SDK reserializes manifest JSON when copying Artifacts, which can change
the manifest digest through annotation key order. Local and remote manifest
digests therefore need not match even when their parsed contents are equal.
When checking a downloaded model, compare its Instance layer digest with the
publication record; check the remote manifest digest against that record
separately.
