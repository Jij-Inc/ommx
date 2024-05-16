OMMX Artifact
===============

OMMX Artifact is an OCI Artifact of media type `application/org.ommx.v1.artifact`.
OCI Artifact is represented as an [OCI Image manifest](https://github.com/opencontainers/image-spec/blob/v1.1.0/manifest.md).
OMMX Artifact is a collection of `config`, `layers`, and annotations.

- `config` is a JSON blob with the following media types:
    - `application/org.ommx.v1.config+json`
      - TBA
- `layers` consists of the following blobs:
    - `application/org.ommx.v1.solution` blob with the following annotations:
        - `org.ommx.v1.solution.instance`: digest of the instance blob
        - `org.ommx.v1.solution.solver`: digest of the solver blob
        - `org.ommx.v1.solution.parameters`: JSON string of the solver parameters
    - `application/org.ommx.v1.instance` blob with the following annotations:
        - TBA
- Annotations in manifest:
  - TBA
