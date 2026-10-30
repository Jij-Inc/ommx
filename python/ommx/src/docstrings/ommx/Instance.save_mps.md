Write the active formulation to an MPS file.


Use MPS to hand the active model to a tool with a compatible MPS reader.
Use {class}`~ommx.artifact.Artifact` when you also need to retain OMMX labels,
evaluation information, or related results. Writing a file does not run the
receiving solver.

# Export a small model

This example uses an isolated temporary directory. Choose a persistent path for
a file you intend to transfer.

```pycon
>>> from pathlib import Path
>>> from tempfile import TemporaryDirectory
>>> from ommx import Instance
>>> instance = Instance.maximize()
>>> x = instance.new_binary("selected", subscripts=[0])
>>> y = instance.new_binary("selected", subscripts=[1])
>>> instance.objective = 3 * x + 2 * y
>>> _ = instance.add_constraint(x + y <= 1, "at_most_one")
>>> with TemporaryDirectory() as directory:
...     path = Path(directory) / "selection.mps"
...     instance.save_mps(str(path), compress=False)
...     assert path.is_file()
...     restored = Instance.load_mps(str(path))
...     assert len(restored.decision_variables) == 2
...     assert len(restored.constraints) == 1

```

The restored model uses generated variable and constraint names, not the
`selected` and `at_most_one` labels. Read the information boundary below
before choosing this format for storage.

For active special constraints, consider {meth}`~ommx.Instance.lower_special_constraints`
or {meth}`~ommx.Instance.prepare` before exporting. These operations mutate the
Instance; copy it first only if you also need the original object. Lowering alone
does not establish MPS compatibility: the remaining functions and variable kinds
must satisfy the conditions below. Mathematically polynomial expressions can be
stored in an unexpanded form; see {meth}`~ommx.Function.powi`.

# Parameters

- ``path``: Destination file. Missing parent directories are created and
  an existing file is overwritten after model validation succeeds.
- ``compress``: Write gzip-compressed bytes when ``True`` (the default),
  or plain text when ``False``, independently of the filename extension.

# Supported models

The writer accepts both minimization and maximization. The active objective
and active regular constraint functions must be stored as expanded
polynomials of degree at most two. Quadratic terms are written using
``QUADOBJ`` and ``QCMATRIX`` sections, with the coefficient convention
described below. Variables used by the active formulation must have
Binary, Integer, or Continuous kind. Active Indicator, OneHot, and SOS1
constraints are rejected; lower them before export if appropriate.
No preparation is performed automatically.

Unsupported models raise ``RuntimeError`` before creating directories
or truncating the destination. Errors reported during path setup or
writing raise ``RuntimeError``. Writing is not atomic: created directories
and a truncated or partial destination may remain after failure.
With compression enabled, the current implementation does not propagate
errors that occur only while finishing the gzip stream. A normal return
therefore does not guarantee a complete gzip file. The instance is not
modified.

# Quadratic coefficient convention

The current writer emits each stored quadratic monomial coefficient once,
unchanged, in both sections. It does not double diagonal objective
coefficients or add mirrored off-diagonal constraint entries.
Receiving tools can therefore interpret a different model or reject the
file even when they recognize these section names.

For example, [Gurobi's MPS convention](https://docs.gurobi.com/projects/optimizer/en/current/reference/fileformats/modelformats.html#quadobj-section)
applies a one-half multiplier to the objective matrix and requires a
symmetric constraint matrix. An objective ``6 * x * x`` is written here
as a diagonal coefficient of ``6``, which that objective convention
interprets as ``3 * x * x``. Check the receiving tool's conventions and
imported quadratic expressions before solving; this writer does not
adapt the coefficients to a selected external tool.

# Information boundary

The writer exports the active objective and sense, active regular
constraints, and the kinds and bounds of variables used by that model.
Quadratic coefficients follow the convention above.
Unused variables, including those referenced only by output semantics,
named functions, or removed constraints, are omitted. Unused variables
of unsupported kinds do not prevent export.

``description.name`` supplies the problem name. Variables and constraints
use generated names ``OMMX_VAR_<id>`` and ``OMMX_CONSTR_<id>``; their
modeling names, subscripts, and other labels are not preserved. Other
Instance information is not exported, including remaining description
fields, annotations, parameters, named functions, removed constraints,
dependencies, fixed-value recovery, and {attr}`~ommx.Instance.output_objective`.
MPS is therefore not a round-trip storage format for an OMMX Instance.
Retain the OMMX instance when its recovery and output semantics are needed
to interpret an external tool's candidates.
