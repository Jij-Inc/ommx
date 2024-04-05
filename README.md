OMMX adaptor for Python-MIP
============================

This package provides an adaptor for the [Python-MIP](https://www.python-mip.com/) from/to [OMMX](https://github.com/Jij-Inc/ommx)

Python-MIP as a solver in OMMX toolchain
-----------------------------------------
```mermaid
sequenceDiagram
    participant O as Other OMMX toolchain
    participant A as Adapter
    participant P as Python-MIP
    O->>A: ommx::Instance and Parameters for Python-MIP;
    A->>P: Translate into Python-MIP input
    P->>P: Solve with CBC, Gurobi, or other solvers
    P->>A: Solution
    A->>O: ommx:Solution
```

Python-MIP as a user interface to create OMMX instance
-------------------------------------------------------
TBW
