from ommx.artifact import Artifact
from ommx._ommx_rust import Descriptor


def test_serialize():
    artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
    for layer in artifact.layers:
        d = layer.to_dict()
        assert layer == Descriptor.from_dict(d)
        json = layer.to_json()
        assert layer == Descriptor.from_json(json)
