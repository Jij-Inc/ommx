from .adapter import OMMXOpenJijSAAdapter as _OMMXOpenJijSAAdapter
from ._decode import decode_to_samples


class OMMXOpenJijSAAdapter(_OMMXOpenJijSAAdapter):
    """Sample optimization candidates with OpenJij simulated annealing.

    Install ``ommx-openjij-adapter`` alongside a compatible OMMX SDK. The example
    below needs no other Adapter, external data, or earlier Notebook state.
    :attr:`INPUT_CLASS` describes the exact input accepted by the constructor
    and preparation-free methods: used decision variables must be Binary, the
    objective must be a polynomial minimization, and no regular or special
    constraints may remain active. Polynomial degree is not limited to two:
    OpenJij's QUBO and Binary-HUBO paths are supported.

    :meth:`sample` returns an evaluated :class:`ommx.SampleSet`; :meth:`solve`
    selects its best feasible candidate as a :class:`ommx.Solution`. Both easy
    paths prepare an isolated copy with a fresh recommended policy. To choose
    a penalty weight, prepare an Instance explicitly and call
    :meth:`sample_without_preparation` or :meth:`solve_without_preparation`.

    **Build a problem and choose its penalty weight**

    Choose exactly one of two options and maximize its score. A OneHot
    constraint expresses that exactly one of the Binary variables is one:

    >>> import copy
    >>> from ommx import FixedPenaltyPreparation, Instance, OneHotConstraint, Sense
    >>> from ommx_openjij_adapter import OMMXOpenJijSAAdapter
    >>> source = Instance.maximize()
    >>> x = [source.new_binary("x", subscripts=[i]) for i in range(2)]
    >>> source.objective = 3 * x[0] + 2 * x[1]
    >>> _ = source.add_one_hot_constraint(OneHotConstraint(variables=x))
    >>> target = OMMXOpenJijSAAdapter.INPUT_CLASS
    >>> target.contains(source)
    False

    The source is a constrained maximization, so it is not a direct OpenJij
    input. Take the Adapter's recommended policy and add an application-chosen
    penalty weight. The recommendation deliberately leaves that weight unset.
    :meth:`ommx.Instance.prepare` changes its receiver in place; retain a copy
    of the source when trying different choices:

    >>> prepared = copy.copy(source)
    >>> policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
    >>> policy.fixed_penalty = (
    ...     FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
    ... )
    >>> prepared.prepare(target, policy)
    >>> target.contains(prepared)
    True
    >>> assert not target.contains(source)

    Here preparation lowers OneHot to an equality, changes maximization to
    minimization, and penalizes the equality violation with weight 4. Choosing
    both options gains two points over choosing the first alone, but incurs a
    penalty of four. This weight is suitable for this small example; it is not
    a general recipe for choosing sufficient penalties. See
    :meth:`ommx.FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight`
    for the exact penalty contract, including its treatment of inequalities.

    Reaching :attr:`INPUT_CLASS` establishes the representation accepted by
    OpenJij. It does not guarantee that sampling will find a feasible candidate
    or prove optimality for the original problem. To change the weight, prepare
    a new copy of ``source``. Changing ``policy`` and preparing the already
    applicable ``prepared`` object does not replace the penalty already stored.

    **Sample and read evaluated candidates**

    Pass the explicitly prepared input to the preparation-free method.
    ``num_reads`` is the number of sampling trials; ``seed`` controls the
    sampler's random input. The candidate distribution can vary with the
    backend version and execution environment:

    >>> sample_set = OMMXOpenJijSAAdapter.sample_without_preparation(
    ...     prepared, num_reads=10, seed=42
    ... )
    >>> summary = sample_set.summary
    >>> assert len(summary) == 10
    >>> assert sample_set.sense == Sense.Maximize

    Inspect ``summary`` to see one row per ``sample_id``, with ``objective`` and
    ``feasible`` columns. Different IDs may have identical assignments.
    Feasible candidates appear first, then the reported objective orders each
    feasibility group according to the problem's reporting sense. In this
    example, the reported objective is the original score
    ``3*x[0] + 2*x[1]`` and larger is better. Feasibility includes the retained
    OneHot condition, even though OpenJij receives no active constraints.

    The table is not raw OpenJij energy: sense conversion, penalties, and a
    constant omitted from the sampler coefficients can make the search energy
    differ from the reported objective. Decoding evaluates candidates through
    the prepared Instance, including its retained evaluation information.
    ``extract_decision_variables("x")`` selects variables named ``x``; dictionary
    keys ``(0,)`` and ``(1,)`` are the subscripts assigned while modeling:

    >>> for sample_id in sample_set.sample_ids():
    ...     candidate = sample_set.get(sample_id)
    ...     values = candidate.extract_decision_variables("x")
    ...     assert candidate.objective == 3 * values[(0,)] + 2 * values[(1,)]
    ...     assert candidate.feasible == (sum(values.values()) == 1)

    **Select among the obtained feasible candidates**

    :attr:`ommx.SampleSet.best_feasible` selects by the reported objective among
    the feasible candidates actually obtained. The names and subscripts set
    while modeling identify the selected values:

    >>> solution = sample_set.best_feasible
    >>> selected = solution.extract_decision_variables("x")
    >>> assert solution.feasible and sum(selected.values()) == 1
    >>> assert solution.objective == 3 * selected[(0,)] + 2 * selected[(1,)]
    >>> details = solution.decision_variables_df()[["name", "subscripts", "value"]]

    This selection is not a global optimality certificate. If no sampled
    candidate is feasible, ``best_feasible`` raises :class:`ValueError`.
    Inspect the feasibility column and reconsider the model, weight, or
    sampling settings. :meth:`solve_without_preparation` performs the same
    sampling and selection in one call and can raise the same error; use the
    sampling path when you need the unsuccessful candidates too.

    Calling :meth:`sample` or :meth:`solve` obtains a new recommended policy;
    neither accepts the edited ``policy`` above. Explicit preparation followed
    by a preparation-free method keeps the application's penalty choice.
    The :class:`ommx.SampleSet` Reference explains result inspection and
    selection in full. For an independent application, see the
    :doc:`TSP sampling example </tutorial/tsp_sampling_with_openjij_adapter>`.
    """


__all__ = [
    "OMMXOpenJijSAAdapter",
    "decode_to_samples",
]
