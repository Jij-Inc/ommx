# What OMMX lets you reuse

OMMX defines solver-independent semantics for optimization problems and candidates, and provides reusable operations on that common model.
Its value extends beyond exchanging files: problem evaluation, supported transformations, value reconstruction,
and connections to execution backends can be reused across integrations.

## Even when you keep using one modeler

You may want to use a model with another backend, check candidates consistently,
or transform constraints that a backend does not support directly. A common OMMX representation
reduces the need for each integration to implement the same transformations and result interpretation independently.
Frequently switching modelers is not a prerequisite for benefiting from OMMX.

| Component | Main responsibility |
|---|---|
| Modeler or domain application | Define domain meaning, input data, and problem structure, and map them to an OMMX problem |
| OMMX core | Provide common semantics for variables, expressions, constraints, and candidate evaluation, plus transformations and reconstruction with public contracts |
| Adapter | Declare direct input, encode backend input, and evaluate returned candidates in OMMX |
| Backend | Search, solve, or sample, returning results and termination information appropriate to its execution method |
| User | Select transformations and settings, and judge whether their conditions and guarantees suffice for the task |

For example, OneHot is an OMMX condition meaning “choose exactly one of these Binary variables.”
Its meaning is independent of any particular solver's API name. Pass it directly to a supporting backend
or lower it to an ordinary equality when needed. See [Special constraints](../user_guide/special_constraints.md)
for semantics and [Transform an Instance](../how_to/transform_instance_without_solver.md) for a procedure.

## Retain the problem while choosing an execution representation

Thinking of OMMX as an “LLVM for mathematical optimization” helps separate source problem structure,
reusable transformations, and connections to execution backends. It does not mean that OMMX already
automatically selects arbitrary transformation sequences or provides a general semantics-preserving presolve system.

This Book's public b7 examples cover Instance construction and evaluation, special constraints,
Preparation toward an InstanceClass, supported transformations, and Adapter execution.
Different transformations preserve different things. [Transformation guarantees](../user_guide/transformation_guarantees.md)
distinguishes feasibility, objective values, and optimality.

More domain-specific conditions and presolve integration that selects and repeats simplification operations
are future directions. See [Versions and environments](compatibility.md) for current functionality and environments.

## Use only the layers you need

Artifact and Experiment are not required for constructing or transforming problems, evaluating candidates,
or executing Adapters. Add Artifact to save and exchange problems or results, or Experiment to organize
conditions and data associated with trials.

- Start in Python: follow [Installation](install.md), then choose a Tutorial or How-to for your goal.
- Connect Rust or another language/tool: use the [Integration Developer Guide](../developer_guide/integration.md)
  for common concepts and version-matched APIs and specifications. Python Tutorials are not prerequisites.
- Check exact operation conditions: use [Reference](../reference/index.md).
