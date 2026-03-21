---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

```{warning}
This document was written for the OMMX Python SDK 1.7.0 release and is not compatible with Python SDK 2.0.0 or later.
```

+++

# OMMX Python SDK 1.7.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.7.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.7.0)

Please refer to the GitHub Release for individual changes.

Summary
--------
- [English Jupyter Book](https://jij-inc.github.io/ommx/en/introduction.html)
- QPLIB format parser
- Several APIs have been added to `ommx.v1.SampleSet` and `ommx.v1.ParametricInstance`, and integration with OMMX Artifact has been added.
  - For `ommx.v1.SampleSet`, please refer to the [new explanation page](https://jij-inc.github.io/ommx/en/ommx_message/sample_set.html)
  - For support of OMMX Artifact, please refer to the API reference [ommx.artifact.Artifact](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact) and [ommx.artifact.ArtifactBuilder](https://jij-inc.github.io/ommx/python/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder).
- Change in behavior of `{Solution, SampleSet}.feasible`

+++

QPLIB format parser
---------------------------

Following the MPS format, support for the QPLIB format parser has been added.

```{code-cell} ipython3
import tempfile

# Example problem from QPLIB
#
# Furini, Fabio, et al. "QPLIB: a library of quadratic programming instances." Mathematical Programming Computation 11 (2019): 237-265 pages 42 & 43
# https://link.springer.com/article/10.1007/s12532-018-0147-4
contents = """
! ---------------
! example problem
! ---------------
MIPBAND # problem name
QML # problem is a mixed-integer quadratic program
Minimize # minimize the objective function
3 # variables
2 # general linear constraints
5 # nonzeros in lower triangle of Q^0
1 1 2.0 5 lines row & column index & value of nonzero in lower triangle Q^0
2 1 -1.0 |
2 2 2.0 |
3 2 -1.0 |
3 3 2.0 |
-0.2 default value for entries in b_0
1 # non default entries in b_0
2 -0.4 1 line of index & value of non-default values in b_0
0.0 value of q^0
4 # nonzeros in vectors b^i (i=1,...,m)
1 1 1.0 4 lines constraint, index & value of nonzero in b^i (i=1,...,m)
1 2 1.0 |
2 1 1.0 |
2 3 1.0 |
1.0E+20 infinity
1.0 default value for entries in c_l
0 # non default entries in c_l
1.0E+20 default value for entries in c_u
0 # non default entries in c_u
0.0 default value for entries in l
0 # non default entries in l
1.0 default value for entries in u
1 # non default entries in u
2 2.0 1 line of non-default indices and values in u
0 default variable type is continuous
1 # non default variable types
3 2 variable 3 is binary
1.0 default value for initial values for x
0 # non default entries in x
0.0 default value for initial values for y
0 # non default entries in y
0.0 default value for initial values for z
0 # non default entries in z
0 # non default names for variables
0 # non default names for constraints"#;
"""

# Create a named temporary file
with tempfile.NamedTemporaryFile(delete=False, suffix='.qplib') as temp_file:
    temp_file.write(contents.encode())
    qplib_sample_path = temp_file.name


print(f"QPLIB sample file created at: {qplib_sample_path}")
```

```{code-cell} ipython3
from ommx import qplib

# Load a QPLIB file
instance = qplib.load_file(qplib_sample_path)

# Display decision variables and constraints
display(instance.decision_variables)
display(instance.constraints)
```

Change in behavior of `{Solution, SampleSet}.feasible`
---------------------

- The behavior of `feasible` in `ommx.v1.Solution` and `ommx.v1.SampleSet` has been changed.
  - The handling of `removed_constraints` introduced in Python SDK 1.6.0 has been changed. In 1.6.0, `feasible` ignored `removed_constraints`, but in 1.7.0, `feasible` now considers `removed_constraints`.
  - Additionally, `feasible_relaxed` which explicitly ignores `removed_constraints` and `feasible_unrelaxed` which considers `removed_constraints` have been introduced. `feasible` is an alias for `feasible_unrelaxed`.


To understand the behavior, let's consider the following simple optimization problem:

$$
\begin{align*}
    \max &\quad x_0 + x_1 + x_2 \\
    \text{s.t.} &\quad x_0 + x_1 \leq 1 \\
                &\quad x_1 + x_2 \leq 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{align*}
$$

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Instance

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + x[1] <= 1).set_id(0),
        (x[1] + x[2] <= 1).set_id(1),
    ],
    sense=Instance.MAXIMIZE,
)
instance.constraints
```

Next, we relax one of the constraints $x_0 + x_1 \leq 1$.

```{code-cell} ipython3
instance.relax_constraint(constraint_id=0, reason="Manual relaxation")
display(instance.constraints)
display(instance.removed_constraints)
```

Now, $x_0 = 1, x_1 = 1, x_2 = 0$ is not a solution to the original problem, but it is a solution to the relaxed problem. Therefore, `feasible_relaxed` will be `True`, but `feasible_unrelaxed` will be `False`. Since `feasible` is an alias for `feasible_unrelaxed`, it will be `False`.

```{code-cell} ipython3
solution = instance.evaluate({0: 1, 1: 1, 2: 0})
print(f"{solution.feasible=}")
print(f"{solution.feasible_relaxed=}")
print(f"{solution.feasible_unrelaxed=}")
```
