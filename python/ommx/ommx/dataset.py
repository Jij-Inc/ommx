from __future__ import annotations

from .artifact import Artifact
from . import v1, _ommx_rust


def miplib2017(name: str) -> v1.Instance:
    """
    Load a MIPLIB 2017 instance as OMMX Artifact.

    >>> from ommx.dataset import miplib2017
    >>> instance = miplib2017("air05")  # doctest: +SKIP

    Common annotations

    >>> instance.title  # doctest: +SKIP
    'air05'
    >>> instance.authors  # doctest: +SKIP
    ['G. Astfalk']
    >>> instance.license  # doctest: +SKIP
    'CC-BY-SA-4.0'
    >>> instance.num_variables  # doctest: +SKIP
    7195
    >>> instance.num_constraints  # doctest: +SKIP
    426

    MIPLIB-specific annotations are stored with `org.ommx.miplib.*` keys.

    >>> instance.annotations["org.ommx.miplib.binaries"]  # doctest: +SKIP
    '7195'
    >>> instance.annotations["org.ommx.miplib.integers"]  # doctest: +SKIP
    '0'
    >>> instance.annotations["org.ommx.miplib.continuous"]  # doctest: +SKIP
    '0'
    >>> instance.annotations["org.ommx.miplib.non_zero"]  # doctest: +SKIP
    '52121'

    >>> instance.annotations["org.ommx.miplib.status"]  # doctest: +SKIP
    'easy'
    >>> instance.annotations["org.ommx.miplib.objective"]  # doctest: +SKIP
    '26374'
    >>> instance.annotations["org.ommx.miplib.url"]  # doctest: +SKIP
    'https://miplib.zib.de/instance_details_air05.html'
    >>> instance.annotations["org.ommx.miplib.tags"]  # doctest: +SKIP
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
    >>> instance = qplib("0018")  # doctest: +SKIP

    Common annotations

    >>> instance.title  # doctest: +SKIP
    'QPLIB_0018'
    >>> instance.authors  # doctest: +SKIP
    ['Andrea Scozzari']
    >>> instance.license  # doctest: +SKIP
    'CC-BY-4.0'
    >>> instance.num_variables  # doctest: +SKIP
    50
    >>> instance.num_constraints  # doctest: +SKIP
    2

    QPLIB-specific annotations are stored with `org.ommx.qplib.*` keys.

    >>> instance.annotations["org.ommx.qplib.nvars"]  # doctest: +SKIP
    '50'
    >>> instance.annotations["org.ommx.qplib.ncons"]  # doctest: +SKIP
    '1'
    >>> instance.annotations["org.ommx.qplib.objtype"]  # doctest: +SKIP
    'quadratic'
    >>> instance.annotations["org.ommx.qplib.objcurvature"]  # doctest: +SKIP
    'indefinite'
    >>> instance.annotations["org.ommx.qplib.probtype"]  # doctest: +SKIP
    'QCL'
    >>> instance.annotations["org.ommx.qplib.url"]  # doctest: +SKIP
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
