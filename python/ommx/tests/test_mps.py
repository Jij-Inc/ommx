from pathlib import Path

import ommx.mps
from ommx.v1 import Instance, DecisionVariable


def test_example_mps():
    test_dir = Path(__file__).parent
    instance = ommx.mps.load_file(str(test_dir / "objsense_max.mps.gz"))

    assert instance.raw.sense == Instance.MAXIMIZE  # OBJSENSE field is specified
    # convert to a format easier to test.
    # for some reason a simple to_dict gets us weird tuple keys so
    # transforming the structure to make it simpler.
    dvars = [
        {k[0]: v for k, v in c.items()}
        for c in instance.decision_variables.to_dict("records")
    ]
    dvars.sort(key=lambda c: c["name"])
    constraints = [
        {k[0]: v for k, v in c.items()} for c in instance.constraints.to_dict("records")
    ]
    constraints.sort(key=lambda c: c["name"])

    assert len(dvars) == 3
    assert len(constraints) == 3
    x, y, z = dvars
    assert x["name"] == "x"
    assert x["kind"] == "continuous"
    assert x["lower"] == 0
    assert x["upper"] == 3
    assert x["subscripts"] == []
    assert y["name"] == "y"
    assert y["kind"] == "continuous"
    assert y["lower"] == 0
    assert y["upper"] == 5
    assert y["subscripts"] == []
    assert z["name"] == "z"
    assert z["kind"] == "continuous"
    assert z["lower"] == 0
    assert z["upper"] == 10
    assert z["subscripts"] == []
    # constr1
    constr1 = constraints[0]
    assert constr1["name"] == "constr1"
    # ids are unstable as of the initial implementation so we can't assert the correct ones are used.
    assert len(constr1["used_ids"]) == 2
    assert constr1["type"] == "linear"
    assert constr1["equality"] == "<=0"
    # constr2
    constr2 = constraints[1]
    assert constr2["name"] == "constr2"
    assert len(constr2["used_ids"]) == 3
    assert constr2["type"] == "linear"
    assert constr2["equality"] == "=0"
    # constr3
    constr3 = constraints[2]
    assert constr3["name"] == "constr3"
    assert len(constr3["used_ids"]) == 2
    assert constr3["type"] == "linear"
    assert constr3["equality"] == "<=0"
