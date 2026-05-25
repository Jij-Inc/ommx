import json

from ommx.experiment import Experiment


def test_serialize():
    experiment = Experiment.with_temp_local_registry()
    experiment.log_json("payload", {"value": 1})
    artifact = experiment.commit()
    for layer in artifact.layers:
        d = layer.to_dict()
        assert d["digest"] == layer.digest
        assert d["size"] == layer.size
        assert d["mediaType"] == layer.media_type
        payload = json.loads(layer.to_json())
        assert payload == d
