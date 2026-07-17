from __future__ import annotations

from collections.abc import Mapping

import openjij as oj
from ommx import Samples, State
from typing_extensions import deprecated


def _decode_to_samples(
    response: oj.Response,
    *,
    variable_ids: set[int] | None = None,
    default_values: Mapping[int, float] | None = None,
) -> Samples:
    """Convert an OpenJij response without evaluating an OMMX Instance."""
    samples = Samples({})
    sample_id = 0
    filtered_defaults = {
        variable_id: value
        for variable_id, value in (default_values or {}).items()
        if variable_ids is None or variable_id in variable_ids
    }

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        entries = dict(filtered_defaults)
        for variable, value in zip(response.variables, sample):
            variable_id = int(variable)  # type: ignore[arg-type]
            if variable_ids is None or variable_id in variable_ids:
                entries[variable_id] = value
        state = State(entries=entries.items())

        # OpenJij does not issue sample IDs. Encode each occurrence as a
        # separate OMMX sample ID while sharing the decoded state.
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        samples.append(ids, state)

    return samples


def decode_to_samples(response: oj.Response) -> Samples:
    """
    Convert `openjij.Response <https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.Response>`_ to :class:`Samples`
    """
    return _decode_to_samples(response)


@deprecated("Renamed to `decode_to_samples`")
def response_to_samples(response: oj.Response) -> Samples:
    """Deprecated: renamed to :func:`decode_to_samples`."""
    return decode_to_samples(response)
