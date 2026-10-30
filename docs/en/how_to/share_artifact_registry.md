# Share an Artifact through a registry

**Create an Artifact with its intended remote name, authenticate, and push it.**
Unlike file exchange, this requires network access and permission to write to
the destination.

This example uses GitHub Container Registry (`ghcr.io`) to distribute a small
JSON note. You can include problems and evaluated results in the same Artifact.
For file exchange without a remote registry, use the complete
{class}`~ommx.artifact.Artifact` example. Experiment is not a prerequisite.

## Environment and permissions

You need the Python SDK from the [installation guide](../getting_started/install.md),
a GitHub account or organization and repository where you are allowed to publish,
and the Docker CLI for login.

Check the intended visibility and access permissions before publishing. Follow
[GitHub's Container Registry authentication instructions](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-to-the-container-registry)
for token scopes and organization SSO requirements. Keep tokens out of Notebooks
and shared source files. Log in from a terminal:

```sh
docker login ghcr.io -u YOUR-GITHUB-USER
```

OMMX uses Docker's credential configuration. Explicit `OMMX_BASIC_AUTH_*`
settings take precedence if present. Recipients do not need login when the
registry permits anonymous reads of the published data.

## Create and send a named Artifact

Replace `my-team`, `my-project`, and `trial-1` with a destination you may write to.
Use a new tag to avoid replacing an existing publication. The example name is
not an OMMX project upload destination.

```python
from ommx.artifact import ArtifactDraft

draft = ArtifactDraft.for_github(
    org="my-team",
    repo="my-project",
    name="candidate-notes",
    tag="trial-1",
)
_ = draft.add_json({"trial": 1, "note": "shared record"}, title="summary")
artifact = draft.commit()
print(artifact.image_name)
artifact.push()
```

`commit()` stores the record locally; `push()` sends it remotely. To include a
problem and result, call `draft.add_instance(instance)` and
`draft.add_solution(solution)` before committing.

{meth}`~ommx.artifact.ArtifactDraft.for_github` also sets repository metadata;
it does not create a GitHub repository or grant access. An automatically named
local Artifact is not a remote publication destination.

## Retrieve the data on the receiving side

Run this on another machine or with a Local Registry that does not yet contain
the reference. Private data requires the recipient to have read access and
credentials. Select the note by the title assigned when it was saved:

```python
from ommx.artifact import Artifact

artifact = Artifact.load("ghcr.io/my-team/my-project/candidate-notes:trial-1")
layer = next(
    layer
    for layer in artifact.layers
    if layer.annotations.get("org.ommx.user.title") == "summary"
)
summary = artifact.get_json(layer)
assert summary == {"trial": 1, "note": "shared record"}
```

`load()` uses a complete local copy without contacting the remote registry. If
the reference or required data is missing, it attempts a fresh remote pull. A
tag with a complete local copy is not refreshed on every call. To identify an
exact publication, use a manifest-digest reference such as
`ghcr.io/.../candidate-notes@sha256:...`, verified by the publisher.
{class}`~ommx.artifact.Artifact` explains names, tags, and content identity.

## If retrieval fails

Check the complete reference, each party's permissions, the login hostname, and
network access. The sender must successfully push the data; a local commit alone
is not sufficient.

{meth}`~ommx.artifact.Artifact.load` documents authentication, authorization,
missing-reference, and transport errors. For local inspection and cleanup,
continue to [Local Registry operations](manage_local_registry.md).
