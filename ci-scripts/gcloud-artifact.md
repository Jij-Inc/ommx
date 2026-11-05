# Google Cloud Artifact Registry integration test

`.github/workflows/gcloud-artifact.yml` runs after pushes to `main` and on manual
dispatch from `main` in `Jij-Inc/ommx`. It builds the current Python SDK, then runs
`gcloud-artifact-roundtrip.py` against the real registry from issue #606.
The local Docker registry tests in `rust/ommx/tests/auth_e2e.rs` still run on PRs.

Each authentication method is checked independently:

- `env`: a short-lived token from `gcloud auth print-access-token`, passed through
  `OMMX_BASIC_AUTH_*`, with an empty Docker configuration.
- `helper`: no `OMMX_BASIC_AUTH_*` overrides; an isolated Docker configuration
  selects `docker-credential-gcloud`.

Each case creates an Instance, Solution, JSON layer, and manifest annotation,
pushes twice, and pulls by tag and digest into separate empty Local Registries.
Manifest digests, complete manifests, layer hashes, annotations, and model
evaluation results must match. The two authentication methods run even if one
fails; any failure fails the job. The job uploads expected manifests and a result
summary for 14 days. Consult the job log for the failing operation.

## Cloud resources and authentication

| Resource | Value |
| --- | --- |
| Project | `ommx-testing` (`166263083727`) |
| Docker repository | `asia-northeast1-docker.pkg.dev/ommx-testing/ommx-artifacts` |
| Workload Identity Pool | `ommx-github` (global) |
| OIDC provider | `artifact-ci` |
| Service account | `ommx-artifact-ci@ommx-testing.iam.gserviceaccount.com` |
| GitHub repository ID | `772371208` |
| GitHub organization ID | `63967332` |

The provider trusts `https://token.actions.githubusercontent.com` and maps:

```text
google.subject=assertion.sub
attribute.repository_id=assertion.repository_id
attribute.repository_owner_id=assertion.repository_owner_id
attribute.ref=assertion.ref
attribute.workflow_ref=assertion.workflow_ref
attribute.event_name=assertion.event_name
```

Its attribute condition is:

```text
attribute.repository_id == '772371208' &&
attribute.repository_owner_id == '63967332' &&
attribute.ref == 'refs/heads/main' &&
attribute.workflow_ref == 'Jij-Inc/ommx/.github/workflows/gcloud-artifact.yml@refs/heads/main' &&
attribute.event_name in ['push', 'workflow_dispatch']
```

The pool principal set
`principalSet://iam.googleapis.com/projects/166263083727/locations/global/workloadIdentityPools/ommx-github/attribute.repository_id/772371208`
has `roles/iam.workloadIdentityUser` on the service account. The service account
has `roles/artifactregistry.writer` on **this repository only**, with no project
role or service account key. The required APIs are `artifactregistry.googleapis.com`,
`iam.googleapis.com`, `iamcredentials.googleapis.com`, and `sts.googleapis.com`.
These resources are provisioned separately from CI; the test cannot change IAM.

The workflow uses
[Google's Workload Identity Federation action](https://github.com/google-github-actions/auth)
and authenticates after building the SDK to preserve the short-lived credential's
lifetime. No GitHub secrets or variables need to be configured. Renaming the
workflow or changing its allowed branch/events also requires updating the cloud
provider's condition. Manual dispatch from other branches is intentionally skipped.

## Retention

Images use `ci-issue-606-env` and `ci-issue-606-helper`, tagged with the GitHub run
ID, attempt, and commit SHA. `gcloud-artifact-cleanup.json` deletes only packages
with the `ci-issue-606-` prefix once they are older than seven days, including
tagged versions. The original manually verified `issue-606-*` images are outside
this policy. Cleanup is asynchronous; it does not happen at an exact deadline.

Apply the policy from the repository root with an administrator account:

```sh
gcloud artifacts repositories set-cleanup-policies ommx-artifacts \
  --project=ommx-testing --location=asia-northeast1 \
  --policy=ci-scripts/gcloud-artifact-cleanup.json --no-dry-run
```

This command replaces the repository's cleanup policies; inspect any existing
policies before reapplying it. See the
[Artifact Registry cleanup documentation](https://docs.cloud.google.com/artifact-registry/docs/repositories/cleanup-policy).

## Run locally

Use an authenticated gcloud account with read/write access to the repository.
From the project root:

```sh
uv sync --locked --package ommx --no-default-groups
uv run --no-sync python ci-scripts/gcloud-artifact-roundtrip.py \
  --repository asia-northeast1-docker.pkg.dev/ommx-testing/ommx-artifacts \
  --output-dir /tmp/ommx-gar-results
```

The default tag is random. Credentials are kept in process environments, while
temporary Docker configurations and Local Registries are removed on exit.
Only test evidence is written to the output directory. Running this script with
a user account verifies transport and authentication discovery, but does not
verify the GitHub OIDC exchange; that requires a run on GitHub Actions.
