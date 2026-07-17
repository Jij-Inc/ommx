from __future__ import annotations

from ommx import Instance, Samples
from typing_extensions import deprecated

from ._decode import decode_to_samples


@deprecated(
    "Use `OMMXOpenJijSAAdapter.sample`; call `prepare` explicitly for transformations"
)
def sample_qubo_sa(
    instance: Instance,
    *,
    beta_min: float | None = None,
    beta_max: float | None = None,
    num_sweeps: int | None = None,
    num_reads: int | None = None,
    schedule: list | None = None,
    initial_state: list | dict | None = None,
    updater: str | None = None,
    sparse: bool | None = None,
    reinitialize_state: bool | None = None,
    seed: int | None = None,
) -> Samples:
    """
    Deprecated: Use :meth:`OMMXOpenJijSAAdapter.sample` instead. This legacy
    helper accepts only the Adapter's direct Binary unconstrained minimization
    input; call :meth:`OMMXOpenJijSAAdapter.prepare` and pass its ``input`` to
    the Adapter for explicit transformations.
    """
    # Import lazily so the deprecated helper uses the package-root Adapter
    # class without introducing a package initialization cycle.
    from . import OMMXOpenJijSAAdapter

    sampler = OMMXOpenJijSAAdapter(
        instance,
        beta_min=beta_min,
        beta_max=beta_max,
        num_sweeps=num_sweeps,
        num_reads=num_reads,
        schedule=schedule,
        initial_state=initial_state,
        updater=updater,
        sparse=sparse,
        reinitialize_state=reinitialize_state,
        seed=seed,
    )
    response = sampler._sample()
    return decode_to_samples(response)
