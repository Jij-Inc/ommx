from pathlib import Path

import ommx.mps
from ommx.v1 import Instance, DecisionVariable, Constraint, Function


test_dir = Path(__file__).parent


def test_example_mps():
    instance = ommx.mps.load_file(str(test_dir / "objsense_max.mps.gz"))

    assert instance.raw.sense == Instance.MAXIMIZE  # OBJSENSE field is specified
    dvars = instance.decision_variables
    dvars.sort(key=lambda x: x.name)
    constraints = instance.constraints
    constraints.sort(key=lambda c: c.name or "")

    assert len(dvars) == 3
    assert len(constraints) == 3
    x, y, z = dvars
    assert x.name == "x"
    assert x.kind == DecisionVariable.CONTINUOUS
    assert x.bound.lower == 0
    assert x.bound.upper == 3
    assert x.subscripts == []
    assert y.name == "y"
    assert y.kind == DecisionVariable.CONTINUOUS
    assert y.bound.lower == 0
    assert y.bound.upper == 5
    assert y.subscripts == []
    assert z.name == "z"
    assert z.kind == DecisionVariable.CONTINUOUS
    assert z.bound.lower == 0
    assert z.bound.upper == 10
    assert z.subscripts == []
    # constr1
    constr1 = constraints[0]
    assert constr1.name == "constr1"
    assert constr1.function.almost_equal(Function(x + y - 4.0))
    assert constr1.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
    # constr2
    constr2 = constraints[1]
    assert constr2.name == "constr2"
    assert constr2.function.almost_equal(Function(x + 2 * y + z - 7))
    assert constr2.equality == Constraint.EQUAL_TO_ZERO
    # constr3
    constr3 = constraints[2]
    assert constr3.name == "constr3"
    assert constr3.function.almost_equal(Function(-z - 2 * x + 10))
    assert constr3.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO


def test_output():
    test_out_file = str(test_dir / "test_mps_output.mps.gz")
    x = [DecisionVariable.binary(i, name=f"d{i}") for i in range(6)]
    obj_coeff = [20, 37, 4, 48, 33, 13]
    constr_coeffs = [
        [10, 85, 12, 17, 35, 78],
        [65, 43, 1, 89, 42, 51],
        [50, 66, 67, 7, 5, 19],
        [73, 33, 47, 8, 90, 2],
        [15, 44, 33, 82, 13, 27],
    ]

    objective = sum(obj_coeff[i] * x[i] for i in range(6)) + 10
    constraints = [
        (sum(constr_coeffs[c][i] * x[i] for i in range(6)) <= 500).add_name(  # type: ignore[reportAttributeAccessIssue]
            f"constr{c}"
        )
        for c in range(5)
    ]

    # Step 4: Create the Instance
    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=constraints,
        sense=Instance.MAXIMIZE,
    )

    instance.write_mps(test_out_file)
    loaded = ommx.mps.load_file(test_out_file)

    # convert to a format easier to test.
    dvars_before = instance.decision_variables
    dvars_after = loaded.decision_variables
    assert len(dvars_before) == len(dvars_after)
    for (b, a) in zip(dvars_before, dvars_after):
        assert b.id == a.id
        # Note: MPS format does not preserve variable names (see Instance.write_mps docstring)
        # assert b.name == a.name  # Skip name check as it's not preserved
        assert b.kind == a.kind
        assert b.bound.lower == a.bound.lower
        assert b.bound.upper == a.bound.upper
        assert b.subscripts == a.subscripts

    constr_before = instance.constraints
    constr_after = loaded.constraints
    assert len(constr_before) == len(constr_after)
    for (b, a) in zip(constr_before, constr_after):
        assert b.id == a.id
        # Note: MPS format does not preserve constraint names (see Instance.write_mps docstring)
        # assert before.name == after.name  # Skip name check as it's not preserved
        assert b.equality == a.equality
        assert b.subscripts == a.subscripts
        assert b.function.almost_equal(a.function)

    # same as above for objective function
    assert instance.objective.almost_equal(loaded.objective)
