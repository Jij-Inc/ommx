import doctest
import importlib
import inspect
import pkgutil

import ommx
import pytest


PUBLIC_CONTRACT_OWNERS = (
    "Instance",
    "Instance.sense",
    "Instance.objective",
    "Instance.to_v1_bytes",
    "Instance.to_v2_bytes",
    "Instance.required_ids",
    "Instance.as_qubo_format",
    "Instance.as_hubo_format",
    "Instance.to_qubo",
    "Instance.to_hubo",
    "Instance.as_parametric_instance",
    "Instance.penalty_method",
    "Instance.uniform_penalty_method",
    "Instance.evaluate",
    "Instance.populate_state",
    "Instance.partial_evaluate",
    "Instance.evaluate_samples",
    "Instance.log_encode",
    "Instance.unary_encode",
    "Instance.substitute",
    "Instance.as_minimization_problem",
    "Instance.as_maximization_problem",
    "Instance.convert_active_objective",
    "Instance.map_active_optimality",
    "Instance.reduce_binary_power",
    "ParametricInstance.to_v1_bytes",
    "ParametricInstance.to_v2_bytes",
    "ParametricInstance.with_parameters",
    "ParametricInstance.substitute",
    "ObjectivePreparation",
    "PreparationPolicy",
    "PreparationPolicy.for_qubo",
    "PreparationPolicy.for_hubo",
    "Instance.prepare",
    "InstanceClass.qubo",
    "InstanceClass.hubo",
)


def resolve_public_owner(path: str):
    owner = ommx
    for component in path.split("."):
        owner = getattr(owner, component)
    return owner


def test_doctest():
    result = doctest.testmod(ommx, optionflags=doctest.ELLIPSIS)
    assert result.failed == 0
    # type: ignore
    for mod in pkgutil.iter_modules(ommx.__path__):
        if mod.name == "v1":
            continue
        mod = importlib.import_module(f"ommx.{mod.name}")
        result = doctest.testmod(mod, optionflags=doctest.ELLIPSIS)
        assert result.failed == 0


@pytest.mark.parametrize("owner_path", PUBLIC_CONTRACT_OWNERS)
def test_public_contract_doctest(owner_path: str) -> None:
    owner = resolve_public_owner(owner_path)
    doc = inspect.getdoc(owner)
    assert doc is not None
    assert any(
        section in doc for section in ("# Invariants", "# Postconditions", "# Errors")
    )
    test = doctest.DocTestParser().get_doctest(
        doc,
        {},
        f"ommx.{owner_path}",
        f"<docstring ommx.{owner_path}>",
        0,
    )
    assert test.examples
    doctest.DebugRunner(optionflags=doctest.ELLIPSIS).run(test)
