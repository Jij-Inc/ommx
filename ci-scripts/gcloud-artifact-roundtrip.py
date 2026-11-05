"""Exercise the public Python Artifact API against a real Google Cloud registry.

Requires an authenticated gcloud CLI and a pre-existing Docker repository.
Each pull runs in a separate process with an empty Local Registry.
"""

import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

from ommx import Instance
from ommx.artifact import Artifact, ArtifactDraft, list_artifacts


def snapshot(artifact: Artifact) -> dict:
    record = next(
        row for row in list_artifacts() if row.image_name == artifact.image_name
    )
    layers = []
    for layer in artifact.layers:
        blob = artifact.get_blob(layer)
        assert layer.digest == "sha256:" + hashlib.sha256(blob).hexdigest()
        layers.append(
            {
                "digest": layer.digest,
                "size": len(blob),
                "media_type": layer.media_type,
                "annotations": layer.annotations,
            }
        )
    instance = artifact.instance
    assert instance.title == "Google Artifact Registry round trip"
    # Exercise the reconstructed model with both feasible and infeasible states.
    for state, feasible, objective in [
        ({0: 1, 1: 0}, True, 3),
        ({0: 0, 1: 1}, True, 2),
        ({0: 1, 1: 1}, False, 5),
    ]:
        evaluated = instance.evaluate(state)
        assert evaluated.feasible == feasible
        assert evaluated.objective == objective
    assert artifact.solution.feasible and artifact.solution.objective == 3
    return {
        "manifest_digest": record.manifest_digest,
        "manifest": record.manifest,
        "layers": layers,
        "annotations": artifact.annotations,
    }


def worker(action: str, reference: str, evidence_path: str) -> None:
    evidence = Path(evidence_path)
    assert not Path(os.environ["OMMX_LOCAL_REGISTRY_ROOT"]).exists()
    if action == "push":
        instance = Instance.maximize()
        instance.title = "Google Artifact Registry round trip"
        x = instance.new_binary("x")
        y = instance.new_binary("y")
        instance.objective = 3 * x + 2 * y
        instance.add_constraint(x + y <= 1)
        draft = ArtifactDraft.new(reference)
        draft.add_annotation("org.opencontainers.image.description", "GAR CI test")
        draft.add_instance(instance)
        draft.add_solution(instance.evaluate({x.id: 1, y.id: 0}))
        draft.add_json(
            {"test": "gar-roundtrip", "reference": reference}, title="verification"
        )
        artifact = draft.commit()
        evidence.write_text(json.dumps(snapshot(artifact), indent=2) + "\n")
        artifact.push()
        print(f"PASS initial push: {reference}", flush=True)
        artifact.push()
        print(f"PASS repeat push: {reference}", flush=True)
    else:
        artifact = Artifact.load(reference)
        assert snapshot(artifact) == json.loads(evidence.read_text())
        print(
            f"PASS fresh pull, hashes, metadata, model evaluation: {reference}",
            flush=True,
        )


def round_trip(repository: str, tag: str, auth: str, output: Path) -> None:
    reference = f"{repository}/ci-gar-roundtrip-{auth}:{tag}"
    host = repository.split("/", 1)[0]
    env = os.environ.copy()
    for name in (
        "OMMX_BASIC_AUTH_DOMAIN",
        "OMMX_BASIC_AUTH_USERNAME",
        "OMMX_BASIC_AUTH_PASSWORD",
    ):
        env.pop(name, None)
    evidence = output / f"{auth}.json"
    with tempfile.TemporaryDirectory(prefix="ommx-gar-") as directory:
        root = Path(directory)
        docker_config = root / "docker"
        docker_config.mkdir()
        # The explicit-token case has no helper or cached Docker credentials.
        config = {"credHelpers": {host: "gcloud"}} if auth == "helper" else {}
        (docker_config / "config.json").write_text(json.dumps(config))
        env["DOCKER_CONFIG"] = str(docker_config)
        if auth == "env":
            token = subprocess.run(
                ["gcloud", "auth", "print-access-token"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            assert token, "gcloud returned an empty access token"
            env.update(
                OMMX_BASIC_AUTH_DOMAIN=host,
                OMMX_BASIC_AUTH_USERNAME="oauth2accesstoken",
                OMMX_BASIC_AUTH_PASSWORD=token,
            )
        for phase in ("push", "pull-tag", "pull-digest"):
            env["OMMX_LOCAL_REGISTRY_ROOT"] = str(root / phase)
            target = reference
            if phase == "pull-digest":
                digest = json.loads(evidence.read_text())["manifest_digest"]
                target = f"{reference.rsplit(':', 1)[0]}@{digest}"
            subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--worker",
                    "push" if phase == "push" else "pull",
                    target,
                    str(evidence),
                ],
                env=env,
                check=True,
                timeout=180,
            )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True, help="HOST/PROJECT/REPOSITORY")
    parser.add_argument("--tag", default=uuid.uuid4().hex)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    print(f"OMMX {importlib.metadata.version('ommx')}", flush=True)
    failures = []
    for auth in ("env", "helper"):
        try:
            round_trip(args.repository, args.tag, auth, args.output_dir)
        except (subprocess.SubprocessError, AssertionError) as error:
            failures.append(auth)
            print(f"FAIL {auth}: {error}", file=sys.stderr, flush=True)
    (args.output_dir / "result.json").write_text(
        json.dumps({"repository": args.repository, "tag": args.tag, "failed": failures})
        + "\n"
    )
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--worker"]:
        worker(*sys.argv[2:])
    else:
        main()
