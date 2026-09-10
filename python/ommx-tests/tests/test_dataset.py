import os
import subprocess
import sys


def test_miplib2017_uses_versioned_distribution_with_legacy_cache(tmp_path):
    # The registry root is process-global. Isolate both versions of the cache
    # from other tests and from the user's registry.
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            """
from ommx import DecisionVariable, Instance, Kind, Sense
from ommx.artifact import Artifact, ArtifactDraft
from ommx.dataset import miplib2017

name = "neos-2626858-aoos"
legacy_ref = f"ghcr.io/jij-inc/ommx/miplib2017:{name}"
versioned_ref = f"ghcr.io/jij-inc/ommx/v2.7/miplib2017:{name}"
corrected = Instance.from_components(
    sense=Sense.Minimize,
    objective=0,
    decision_variables=[DecisionVariable.binary(506, name="C0506")],
    constraints={},
)
for ref, model in [(legacy_ref, Instance.empty()), (versioned_ref, corrected)]:
    draft = ArtifactDraft.new(ref)
    # Published v2.7 Artifacts have v1 protobuf layers and descriptor annotations.
    draft.add_layer(
        "application/org.ommx.v1.instance",
        model.to_v1_bytes(),
        {"org.ommx.v1.instance.title": name},
    )
    draft.commit()

instance = miplib2017(name)
assert instance.title == name
assert len(instance.decision_variables) == 1
variable = instance.decision_variables[0]
assert variable.id == 506 and variable.name == "C0506"
assert variable.kind == Kind.Binary
assert variable.bound.lower == 0 and variable.bound.upper == 1
assert len(Artifact.load(legacy_ref).instance.decision_variables) == 0
""",
        ],
        env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(tmp_path)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
