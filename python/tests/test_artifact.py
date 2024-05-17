import ommx
from pathlib import Path


def test_from_oci_archive():
    path = Path(__file__).parent / "data" / "random_lp_instance.ommx"
    artifact = ommx.Artifact.from_oci_archive(str(path))
    assert len(artifact.get_instance_descriptors()) == 1
