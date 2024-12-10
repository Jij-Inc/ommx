from __future__ import annotations

from .artifact import Artifact
from . import v1, _ommx_rust


def miplib2017(name: str) -> v1.Instance:
    """
    Load a MIPLIB 2017 instance as OMMX Artifact.

    >>> from ommx.dataset import miplib2017
    >>> instance = miplib2017("air05")

    Common annotations

    >>> instance.title
    'air05'
    >>> instance.authors
    ['G. Astfalk']
    >>> instance.license
    'CC-BY-SA-4.0'
    >>> instance.num_variables
    7195
    >>> instance.num_constraints
    426

    MIPLIB-specific annotations are stored with `org.ommx.miplib.*` keys.

    >>> instance.annotations["org.ommx.miplib.binaries"]
    '7195'
    >>> instance.annotations["org.ommx.miplib.integers"]
    '0'
    >>> instance.annotations["org.ommx.miplib.continuous"]
    '0'
    >>> instance.annotations["org.ommx.miplib.non_zero"]
    '52121'

    >>> instance.annotations["org.ommx.miplib.status"]
    'easy'
    >>> instance.annotations["org.ommx.miplib.objective"]
    '26374'
    >>> instance.annotations["org.ommx.miplib.url"]
    'https://miplib.zib.de/instance_details_air05.html'
    >>> instance.annotations["org.ommx.miplib.tags"]
    'benchmark,binary,benchmark_suitable,set_partitioning'

    """
    artifact = Artifact.load(f"ghcr.io/jij-inc/ommx/miplib2017:{name}")
    return artifact.instance


def miplib2017_instance_annotations() -> dict[str, dict[str, str]]:
    """
    Return MIPLIB 2017 instance annotations.

    >>> from ommx.dataset import miplib2017_instance_annotations
    >>> annotations = miplib2017_instance_annotations()
    >>> sorted(annotations.keys())
    ['10teams', ..., 'air05', ...]
    >>> annotations["air05"]["org.ommx.miplib.status"]
    'easy'

    """
    return _ommx_rust.miplib2017_instance_annotations()
