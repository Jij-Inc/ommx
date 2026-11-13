Explain violated constraints using the saved evaluation tolerance.

```pycon
>>> from ommx import Instance, OneHotConstraint, OneHotViolationDiagnostics
>>> model = Instance.minimize()
>>> x, y = model.new_binary("x"), model.new_binary("y")
>>> choice = model.add_one_hot_constraint(OneHotConstraint(variables=[x, y]))
>>> solution = model.evaluate({x.id: 0, y.id: 0})
>>> diagnostics = solution.violation_diagnostics()
>>> detail = diagnostics[choice.constraint_id]
>>> assert isinstance(detail, OneHotViolationDiagnostics.MissingSelection)
>>> assert detail.values == {x.id: 0, y.id: 0}

```

Returns a read-only {class}`~ommx.ViolationDiagnostics` Mapping. Details are
computed on request from this result's stored variable values and
`feasibility_atol`; no evaluation or canonicalization is repeated. Feasible
constraints are absent. The default `min_role=EVALUATION_ONLY` includes all
retained roles, even skipped checks. `FEASIBILITY_CHECK` includes normal checks
and active constraints; `ACTIVE` includes only active constraints.

Regular and Indicator diagnostics currently have no extra fields. SOS1 reports
excess members with their signed values. OneHot distinguishes missing selection,
multiple selections, and value errors. Returned data is independent of the
result; changing a property dictionary does not change the diagnostic.

An empty Mapping describes only the selected constraints, not variable domains.
Changing a row's query tolerance does not change the saved tolerance used here.
Minor releases may add diagnostic information or OneHot variants; removing or
changing existing information requires a major release.
