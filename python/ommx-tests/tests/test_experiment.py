import pandas as pd
import pytest

from ommx.artifact import ArtifactDraft, set_local_registry_root
from ommx.experiment import Experiment


@pytest.fixture(scope="module", autouse=True)
def isolated_local_registry(tmp_path_factory):
    set_local_registry_root(tmp_path_factory.mktemp("experiment") / "registry")


def test_load_experiment_and_view_run_parameters(isolated_local_registry):
    draft = ArtifactDraft.new_anonymous()
    draft.add_annotation("org.ommx.artifact.kind", "experiment")
    draft.add_annotation("org.ommx.experiment.schema", "v1")
    draft.add_annotation("org.ommx.experiment.status", "finished")
    draft.add_layer(
        "application/json",
        b'{"name": "miplib2017"}',
        {
            "org.ommx.experiment.space": "experiment",
            "org.ommx.record.name": "dataset",
        },
    )
    draft.add_layer(
        "application/json",
        b'{"formulation": "a"}',
        {
            "org.ommx.experiment.space": "run",
            "org.ommx.experiment.run_id": "0",
            "org.ommx.record.name": "candidate",
        },
    )
    draft.add_layer(
        "application/org.ommx.v1.experiment.run-parameters+json",
        b"""
        {
          "columns": {
            "presolve": {
              "type": "bool",
              "values": { "1": true }
            },
            "solver": {
              "type": "string",
              "values": { "0": "scip", "1": "highs" }
            },
            "time_limit": {
              "type": "float64",
              "values": { "0": 20.0 }
            }
          }
        }
        """,
        {"org.ommx.experiment.layer": "run-parameters"},
    )
    artifact = draft.commit()
    image_name = artifact.image_name
    assert image_name is not None

    experiment = Experiment.load(image_name)

    assert experiment.image_name == image_name
    assert {(record.space, record.run_id, record.name) for record in experiment.records} == {
        ("experiment", None, "dataset"),
        ("run", 0, "candidate"),
    }

    df = experiment.run_parameters_df()
    assert list(df.index) == [0, 1]
    assert df.index.name == "run_id"
    assert df.loc[0, "solver"] == "scip"
    assert df.loc[1, "solver"] == "highs"
    assert bool(df.loc[1, "presolve"]) is True
    assert df.loc[0, "time_limit"] == 20.0
    assert pd.isna(df.loc[1, "time_limit"])
