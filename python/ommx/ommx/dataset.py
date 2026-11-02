"""Load published benchmark problems as :class:`ommx.Instance` objects.

:func:`miplib2017` takes a problem name, while :func:`qplib` takes a
numeric tag as a string, including any leading zeros. The first load needs
access to the public registry. Subsequent loads reuse the local Artifact.
Record the distribution reference and the problem name when comparing runs.
The distribution version identifies the data independently of the SDK version.
See the publication records for
`MIPLIB v2.7 <https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md>`_
and `QPLIB v2.8 <https://github.com/Jij-Inc/ommx/blob/b4cffe9f1ce5323c15de67eca55b05d9f87285a0/rust/dataset/distributions/v2.8/README.md>`_
for coverage, corrections and model digests. Dataset annotations describe the
published instance. The :doc:`benchmark solving example </examples/published_benchmarks>`
compares a solver result with the published objective. A known objective value
alone does not establish the feasibility or optimality of a candidate.

For local files, use :meth:`ommx.Instance.load_mps` or :meth:`ommx.Instance.load_qplib`.
:func:`ommx.qplib.load_solution` shows how to read a problem and a matching
candidate, evaluate it and inspect violations without a solver. For a local
OMMX archive, :meth:`ommx.artifact.Artifact.import_archive` imports the data
into the local registry and :meth:`ommx.artifact.Artifact.get_instance` selects
the problem. An Instance supplied by a modeler is ready for the same operations.
"""

from __future__ import annotations

from .artifact import Artifact
from . import Instance, _ommx_rust


def miplib2017(name: str) -> Instance:
    """
    Load a MIPLIB 2017 instance as OMMX Artifact.

    Uses the published ``ghcr.io/jij-inc/ommx/v2.7/miplib2017`` distribution,
    generated with the corrected MPS integer bounds in OMMX 2.7.0. The
    distribution version is fixed independently of the installed SDK version;
    existing unversioned Artifacts are not used as a fallback.

    >>> from ommx.dataset import miplib2017

    Loading the instance requires remote registry access, so the following
    calls are not executed as doctests.

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
    artifact = Artifact.load(f"ghcr.io/jij-inc/ommx/v2.7/miplib2017:{name}")
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


def qplib(tag: str) -> Instance:
    """
    Load a QPLIB instance as OMMX Artifact.

    Uses the published ``ghcr.io/jij-inc/ommx/v2.8/qplib`` distribution,
    generated with corrected quadratic coefficients in OMMX 2.8.0. The
    distribution version is independent of the installed SDK version;
    old unversioned Artifacts are not used as a fallback.

    >>> from ommx.dataset import qplib

    Loading the instance requires remote registry access, so the following
    calls are not executed as doctests.

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
    artifact = Artifact.load(f"ghcr.io/jij-inc/ommx/v2.8/qplib:{tag}")
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
