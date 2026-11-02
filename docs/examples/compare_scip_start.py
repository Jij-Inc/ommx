"""Compare two SCIP runs on roll3000 with the same model and solver settings."""

import json
from pathlib import Path
from time import perf_counter

from ommx import dataset
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


if __name__ == "__main__":
    problem = dataset.miplib2017("roll3000")
    path = Path(__file__).with_name("roll3000-start.json")
    values = {int(i): v for i, v in json.loads(path.read_text()).items()}
    for label, candidate in [("without a start", None), ("with a start", values)]:
        adapter = OMMXPySCIPOptAdapter(problem, initial_state=candidate)
        model = adapter.solver_input
        model.setRealParam("limits/time", 30)
        start = perf_counter()
        model.optimize()
        elapsed = perf_counter() - start
        objective = adapter.decode(model).objective if model.getNSols() else None
        print(f"{label}: {elapsed:.2f} s, status={model.getStatus()}, objective={objective}")
