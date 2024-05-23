import ommx._ommx_rust
from pathlib import Path

DATA_ROOT = Path(__file__).parent.parent.parent / "data"


def test_from_oci_archive():
    path = DATA_ROOT / "random_lp_instance.ommx"
    artifact = ommx._ommx_rust.Artifact.from_oci_archive(str(path))
    assert len(artifact.instance_descriptors) == 1

    desc = artifact.instance_descriptors[0]
    assert (
        desc.digest
        == "sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2"
    )
    assert desc.size == 639
    assert desc.annotations['org.ommx.v1.instance.title'] == "random_lp"
    assert 'org.ommx.v1.instance.created' in desc.annotations
