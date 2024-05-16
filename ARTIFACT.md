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
        - `org.ommx.v1.solution.instance`: The digest of the corresponding instance of the solution
        - `org.ommx.v1.solution.solver`: The digest of the solver information which generated the solution
        - `org.ommx.v1.solution.parameters`: Solver parameters used to generate the solution as a JSON
        - `org.ommx.v1.solution.start`: The start time of the solution as a RFC3339 string
        - `org.ommx.v1.solution.end`: The end time of the solution as a RFC3339 string
    - `application/org.ommx.v1.instance` blob with the following annotations:
        - TBA
- Annotations in manifest:
  - TBA

Note that other annotations listed above are also allowed.
The key may not start with `org.ommx.v1.`, but must be a valid reverse domain name as specified by OCI specification.
