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


def qplib(tag: str) -> v1.Instance:
    """
    Load a QPLIB instance as OMMX Artifact.

    >>> from ommx.dataset import qplib
    >>> instance = qplib("0018")

    Common annotations

    >>> instance.title
    'QPLIB_0018'
    >>> instance.authors
    ['Andrea Scozzari']
    >>> instance.license
    'CC-BY-4.0'
    >>> instance.num_variables
    50
    >>> instance.num_constraints  # QPLIB counts l <= f(x) <= u as 1, OMMX counts as 2
    2

    QPLIB-specific annotations are stored with `org.ommx.qplib.*` keys.

    >>> instance.annotations["org.ommx.qplib.nvars"]
    '50'
    >>> instance.annotations["org.ommx.qplib.ncons"]
    '1'
    >>> instance.annotations["org.ommx.qplib.objtype"]
    'quadratic'
    >>> instance.annotations["org.ommx.qplib.objcurvature"]
    'indefinite'
    >>> instance.annotations["org.ommx.qplib.probtype"]
    'QCL'
    >>> instance.annotations["org.ommx.qplib.url"]
    'http://qplib.zib.de/QPLIB_0018.html'

    """
    artifact = Artifact.load(f"ghcr.io/jij-inc/ommx/qplib:{tag}")
    return artifact.instance


def qplib_instance_annotations() -> dict[str, dict[str, str]]:
    """
    Return QPLIB instance annotations.

    >>> from ommx.dataset import qplib_instance_annotations
    >>> annotations = qplib_instance_annotations()
    >>> len(annotations)
    453
    >>> annotations["0018"]["org.ommx.qplib.probtype"]
    'QCL'

    """
    return _ommx_rust.qplib_instance_annotations()
