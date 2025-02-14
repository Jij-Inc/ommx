# OMMX adapter for Gurobi

This package provides an adapter for [Gurobi](https://www.gurobi.com/) from [OMMX](https://github.com/Jij-Inc/ommx). It allows you to solve optimization problems defined in OMMX format using Gurobi's powerful solver.

## Features

- Support for various variable types:
  - Binary variables
  - Integer variables
  - Continuous variables
- Support for different optimization problems:
  - Linear Programming (LP)
  - Mixed Integer Linear Programming (MILP)
  - Quadratic Programming (QP)
  - Mixed Integer Quadratic Programming (MIQP)
- Support for both minimization and maximization problems

## Prerequisites

- Python >= 3.9
- Gurobi Optimizer and valid license
- gurobipy >= 10.0.0
- ommx >= 1.8.4

## Installation

First, ensure you have Gurobi installed and properly licensed. Then install the OMMX Gurobi adapter using pip:

```bash
pip install ommx-gurobipy-adapter
```

## Usage

Here's a simple example of how to use the adapter:

```python
from ommx_gurobipy_adapter import OMMXGurobipyAdapter
from ommx.v1 import Instance, DecisionVariable

# Create decision variables
x1 = DecisionVariable.integer(1, lower=0, upper=5)
x2 = DecisionVariable.continuous(2, lower=0, upper=5)

# Create OMMX instance
instance = Instance.from_components(
    decision_variables=[x1, x2],
    objective=x1 + 2*x2,
    constraints=[
        x1 + x2 <= 5,  # Linear constraint
    ],
    sense=Instance.MAXIMIZE,
)

# Solve using Gurobi
solution = OMMXGurobipyAdapter.solve(instance)

# Access the results
print(f"Objective value: {solution.objective}")
print(f"x1 = {solution.state.entries[1]}")
print(f"x2 = {solution.state.entries[2]}")
```


### Controlling Gurobi Parameters

If you need more control over the Gurobi solver parameters, you can use the adapter in two steps:

```python
from ommx_gurobipy_adapter import OMMXGurobipyAdapter

# Create adapter
adapter = OMMXGurobipyAdapter(instance)

# Get Gurobi model
model = adapter.solver_input

# Set Gurobi parameters
model.setParam('TimeLimit', 60)  # Set time limit to 60 seconds
model.setParam('MIPGap', 0.01)   # Set relative MIP gap tolerance to 1%

# Solve
model.optimize()

# Get solution
solution = adapter.decode(model)
```

## Error Handling

The adapter provides specific error types for different situations:

- `OMMXGurobipyAdapterError`: Base error class for adapter-specific errors
- `InfeasibleDetected`: Raised when the problem is infeasible
- `UnboundedDetected`: Raised when the problem is unbounded

Example of error handling:

```python
from ommx_gurobipy_adapter import OMMXGurobipyAdapterError
from ommx.adapter import InfeasibleDetected, UnboundedDetected

try:
    solution = OMMXGurobipyAdapter.solve(instance)
except InfeasibleDetected:
    print("Problem is infeasible")
except UnboundedDetected:
    print("Problem is unbounded")
except OMMXGurobipyAdapterError as e:
    print(f"Adapter error: {e}")
```

## Testing

To run the test suite:

```bash
python -m pytest tests/
```

## License

This project is licensed under the Apache License 2.0 - see the LICENSE file for details.

## Reference

For more information about OMMX, please visit: https://github.com/Jij-Inc/ommx

For Gurobi documentation, please visit: https://www.gurobi.com/documentation/

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.