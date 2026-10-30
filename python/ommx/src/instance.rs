#![expect(
    deprecated,
    reason = "PyO3 and pyo3-stub-gen wrappers reference Instance::empty"
)]

use crate::constraint_id::{
    ConstraintIDInput, ConstraintMapInput, ConstraintSetInput, IndicatorConstraintIDInput,
    OneHotConstraintIDInput, Sos1ConstraintIDInput,
};
use crate::{
    constraint_kind::{constraint_kind_collection, ConstraintKind},
    error::OmmxPyResult,
    pandas::{
        apply_include_filter, constraint_id_col, entries_to_dataframe, raw_entries_to_dataframe,
        PyDataFrame, ToPandasEntry,
    },
    Constraint, DecisionVariable, DecisionVariableRole, Function, NamedFunction,
    ParametricInstance, Rng, SampleSet, Samples, Sense, Solution, State,
};
use ommx::{ConstraintID, Evaluate, NamedFunctionID, VariableID};
use pyo3::{
    exceptions::{PyKeyError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict, PyFloat, PyInt, PyString, PyTuple},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

struct NamedStateInput(ommx::NamedState);

impl<'py> FromPyObject<'_, 'py> for NamedStateInput {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let type_error = || {
            PyTypeError::new_err(
                "named values must be a Mapping[str, Mapping[tuple[int, ...], float]]",
            )
        };
        if !value.hasattr("items")? {
            return Err(type_error());
        }
        let mut result = BTreeMap::new();
        for item in value.call_method0("items")?.try_iter()? {
            let (name, values) = item?.extract::<(String, Bound<'py, PyAny>)>()?;
            if result.contains_key(&name) {
                return Err(PyValueError::new_err(format!(
                    "Duplicate variable name after string normalization: {name:?}"
                )));
            }
            if !values.hasattr("items")? {
                return Err(type_error());
            }
            let mut entries = BTreeMap::new();
            for item in values.call_method0("items")?.try_iter()? {
                let (subscripts, value) = item?.extract::<(Bound<'py, PyTuple>, f64)>()?;
                let subscripts = subscripts.extract::<Vec<i64>>()?;
                if entries.contains_key(&subscripts) {
                    return Err(PyValueError::new_err(format!(
                        "Duplicate subscripts after integer normalization for name {name:?}: {subscripts:?}"
                    )));
                }
                entries.insert(subscripts, value);
            }
            result.insert(name, entries);
        }
        Ok(Self(result))
    }
}

impl pyo3_stub_gen::PyStubType for NamedStateInput {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("NamedState", "ommx._ommx_rust".into())
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

enum NamedStateMapping {}

impl pyo3_stub_gen::PyStubType for NamedStateMapping {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "collections.abc.Mapping[str, collections.abc.Mapping[tuple[int, ...], float]]"
                .into(),
            source_module: None,
            import: ["collections.abc".into()].into(),
            type_refs: Default::default(),
        }
    }
}

impl pyo3_stub_gen::runtime::PyRuntimeType for NamedStateMapping {
    fn runtime_type_object(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        let mapping = py.import("collections.abc")?.getattr("Mapping")?;
        let subscripts = py
            .get_type::<PyTuple>()
            .get_item((py.get_type::<PyInt>(), py.Ellipsis()))?;
        let values = mapping.get_item((subscripts, py.get_type::<PyFloat>()))?;
        mapping.get_item((py.get_type::<PyString>(), values))
    }
}

pyo3_stub_gen::type_alias!(
    "ommx._ommx_rust",
    NamedState = NamedStateMapping,
    "Decision-variable values grouped by name and tuple subscripts. Use () for scalars and (i,) for one-dimensional indices. Resolve against the target Instance with state_from_named_values."
);

#[derive(Default)]
struct StringMapping(HashMap<String, String>);

impl<'py> FromPyObject<'_, 'py> for StringMapping {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let type_error = || PyTypeError::new_err("parameters must be a Mapping[str, str]");
        let items = value.call_method0("items").map_err(|_| type_error())?;
        let mut parameters = HashMap::new();
        for item in items.try_iter().map_err(|_| type_error())? {
            let (key, value) = item?
                .extract::<(String, String)>()
                .map_err(|_| type_error())?;
            parameters.insert(key, value);
        }
        Ok(Self(parameters))
    }
}

impl pyo3_stub_gen::PyStubType for StringMapping {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "collections.abc.Mapping[str, str]".to_string(),
            source_module: None,
            import: ["collections.abc".into()].into(),
            type_refs: Default::default(),
        }
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

impl<'py> IntoPyObject<'py> for StringMapping {
    type Target = PyDict;
    type Output = Bound<'py, PyDict>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let mapping = PyDict::new(py);
        for (key, value) in self.0 {
            mapping.set_item(key, value)?;
        }
        Ok(mapping)
    }
}

/// Read-only objective semantics used when evaluating solver output.
///
/// # Invariants
///
/// The sense, function, and optimality-transport flag are exposed as one
/// root-owned value. Instances create and update this value through their
/// mathematical owner operations; Python callers cannot construct or mutate it.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Clone)]
pub struct OutputObjective {
    inner: ommx::OutputObjective,
}

impl From<ommx::OutputObjective> for OutputObjective {
    fn from(inner: ommx::OutputObjective) -> Self {
        Self { inner }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl OutputObjective {
    /// Optimization sense used for evaluated output objective values.
    #[getter]
    pub fn sense(&self) -> Sense {
        self.inner.sense().into()
    }

    /// Function evaluated after the full state is populated.
    #[getter]
    pub fn function(&self) -> Function {
        Function(self.inner.function().clone())
    }

    /// Whether active-formulation optimality transports to the output objective.
    #[getter]
    pub fn preserves_optimality(&self) -> bool {
        self.inner.preserves_optimality()
    }
}

/// An optimization problem with variable domains, an objective, and constraints.
///
/// An Instance defines which assignments are feasible and how their objective
/// values are computed independently of a solver. Its main components are:
///
/// - {attr}`~ommx.Instance.decision_variables`: variables and their domains.
/// - {attr}`~ommx.Instance.objective`: the objective {class}`~ommx.Function`.
/// - {attr}`~ommx.Instance.constraints`: all active regular and special constraints.
/// - {attr}`~ommx.Instance.sense`: whether the objective is minimized or maximized.
///
/// Regular constraints have the form ``f(x) == 0`` or ``f(x) <= 0``;
/// ``f(x) >= 0`` is represented as ``-f(x) <= 0``. Special constraints express
/// structure directly, such as conditional constraints
/// ({class}`~ommx.IndicatorConstraint`), exactly one selected binary variable
/// ({class}`~ommx.OneHotConstraint`), or at most one nonzero variable
/// ({class}`~ommx.Sos1Constraint`). They are conditions of the problem, not
/// instructions to a particular solver. See {class}`~ommx.InstanceClass` for
/// the separate question of which representations a solver adapter accepts.
///
/// **Build and evaluate a problem**
///
/// This problem maximizes ``x + y`` subject to ``x * y == 0``, with binary
/// variables. {meth}`~ommx.Instance.maximize` starts an empty maximization
/// problem; use {meth}`~ommx.Instance.minimize` for minimization.
///
/// >>> from ommx import Instance
/// >>> instance = Instance.maximize()
/// >>> x = instance.new_binary("x")
/// >>> y = instance.new_binary("y")
/// >>> instance.objective = x + y
/// >>> exclusive = instance.add_constraint(x * y == 0, "exclusive")
/// >>> solution = instance.evaluate({x.id: 1, y.id: 0})
/// >>> assert solution.feasible
/// >>> assert solution.objective == 1.0
///
/// The ``new_*`` methods and {meth}`~ommx.Instance.add_constraint` assign IDs
/// automatically and return attached handles. IDs remain accessible through
/// ``x.id`` and ``exclusive.constraint_id``. {meth}`~ommx.Instance.evaluate`
/// accepts a {class}`~ommx.State` or an ID-to-value dictionary and returns a
/// {class}`~ommx.Solution`. Evaluation checks the supplied candidate; it does
/// not search for a better one or establish optimality.
///
/// Use {meth}`~ommx.Instance.from_components` when components already have
/// explicit IDs. The Instance validates that expressions and special
/// constraints refer to variables defined in that Instance.
///
/// >>> from ommx import ConstraintID, DecisionVariable, Instance, Sense
/// >>> a = DecisionVariable.binary(10, name="a")
/// >>> assembled = Instance.from_components(
/// ...     decision_variables=[a], objective=a, constraints={7: a <= 1},
/// ...     sense=Sense.Maximize,
/// ... )
/// >>> assert ConstraintID(7) in assembled.constraints
/// >>> assert assembled.evaluate({a.id: 1}).feasible
///
/// **Variable domains and modeling labels**
///
/// A variable's ``kind``, ``lower``, and ``upper`` define its domain. Binary
/// variables take 0 or 1, Integer variables take integers within their bounds,
/// and Continuous variables take real values within their bounds.
/// SemiInteger and SemiContinuous additionally allow zero even when zero is
/// outside the specified interval.
///
/// >>> from ommx import Instance
/// >>> typed = Instance.minimize()
/// >>> flag = typed.new_binary("flag")
/// >>> count = typed.new_integer("count", lower=0, upper=10)
/// >>> amount = typed.new_continuous("amount", lower=0)
/// >>> batch = typed.new_semi_integer("batch", lower=2, upper=10)
/// >>> rate = typed.new_semi_continuous("rate", lower=0.5, upper=4)
///
/// Here ``batch`` permits 0 and the integers from 2 through 10; ``rate``
/// permits 0 and real values from 0.5 through 4. Each ``new_*`` method
/// documents its bound normalization, tolerance, and validation rules. An
/// invalid definition is rejected before adding the variable or its label.
///
/// Variables and constraints also carry ``name``, ``subscripts``,
/// ``parameters``, and ``description`` labels. For example, ``name="x"``
/// and ``subscripts=[i]`` describe a source-model variable ``x_i``;
/// ``parameters`` stores string-valued indices. Labels support display and
/// analysis without changing the mathematical conditions. Names need not be
/// unique: identity is determined by IDs, not names or labels.
/// All ``new_*`` methods and {meth}`~ommx.Instance.add_constraint` accept
/// these labels. For ``add_constraint``, omitted labels preserve the input
/// constraint's labels.
///
/// {meth}`~ommx.Instance.decision_variables_df` collects definitions and
/// labels into a pandas DataFrame. {meth}`~ommx.Instance.format_function` and
/// {meth}`~ommx.Instance.display_function` use the Instance's variable labels
/// to display expressions; a Function alone retains only variable IDs.
/// After evaluation, {meth}`~ommx.Solution.extract_decision_variables` uses
/// names and subscripts to retrieve values for application-level analysis.
///
/// **Constraint identity and access**
///
/// Constraint numbers are unique across all families in one Instance,
/// including removed constraints. Their numbering is separate from variable
/// IDs. Supply distinct numbers across the regular and special-constraint
/// mappings passed to {meth}`~ommx.Instance.from_components`.
///
/// {attr}`~ommx.Instance.constraints` is a live collection of all active
/// families. A typed ID identifies both the family and the number. Bare
/// integers are rejected by this heterogeneous collection even though
/// constraint numbers are globally unique within the Instance. Obtaining
/// the collection does not collect rows; each row is fetched when accessed.
/// {attr}`~ommx.Instance.removed_constraints` exposes retained constraints
/// that are no longer active.
///
/// >>> from ommx import ConstraintID, Instance
/// >>> model = Instance.minimize()
/// >>> selected = model.new_binary("selected")
/// >>> added = model.add_constraint(selected == 1, "required")
/// >>> assert added.constraint_id == ConstraintID(0)
/// >>> assert model.constraints[added.constraint_id].name == "required"
/// >>> assert list(model.constraints) == [added.constraint_id]
///
/// Use {meth}`~ommx.Instance.constraints_df` with an explicit ``kind``, such
/// as ``kind="regular"``, to collect one family into a DataFrame. Collection
/// lookup and iteration contracts belong to {class}`~ommx.ConstraintCollection`.
///
/// **Edit the Instance through attached handles**
///
/// {attr}`~ommx.Instance.decision_variables`,
/// {meth}`~ommx.Instance.attached_decision_variable`, and active constraint
/// lookups return attached handles. Their label setters update the owning
/// Instance. The following example updates a variable and a regular constraint
/// after construction; the same operations work on a loaded Instance.
/// ``set_name``, ``set_description``, ``set_subscripts``, and ``set_parameters``
/// replace their fields; ``add_subscripts`` appends indices and
/// ``add_parameter`` / ``add_parameters`` merge parameter entries.
///
/// >>> from ommx import Instance
/// >>> labeled = Instance.maximize()
/// >>> choice = labeled.new_binary("choice")
/// >>> labeled.objective = choice
/// >>> rule = labeled.add_constraint(choice <= 1, "limit")
/// >>> variable = labeled.attached_decision_variable(choice.id)
/// >>> constraint = labeled.constraints[rule.constraint_id]
/// >>> variable.set_name("selected")
/// >>> variable.set_subscripts([0])
/// >>> variable.set_description("Whether product 0 is selected")
/// >>> constraint.set_name("selection_limit")
/// >>> assert labeled.attached_decision_variable(choice.id).name == "selected"
/// >>> assert labeled.constraints[rule.constraint_id].name == "selection_limit"
///
/// Changing labels preserves IDs and mathematical conditions. Set labels
/// before evaluation when the resulting Solution should carry them. By
/// contrast, ``detach()`` and
/// {meth}`~ommx.Instance.get_decision_variable_by_id` return independent
/// snapshots. Editing a snapshot or a DataFrame does not write back to the
/// Instance.
///
/// >>> snapshot = constraint.detach()
/// >>> _ = snapshot.set_name("snapshot_only")
/// >>> table = labeled.decision_variables_df()
/// >>> table.loc[choice.id, "name"] = "table_only"
/// >>> assert labeled.constraints[rule.constraint_id].name == "selection_limit"
/// >>> assert labeled.attached_decision_variable(choice.id).name == "selected"
///
/// **Retained variables and output evaluation**
///
/// {attr}`~ommx.Instance.decision_variables` contains every retained variable.
/// {attr}`~ommx.Instance.used_decision_variables` selects variables used in
/// the current objective or active constraints. Fixed and dependent variables
/// remain defined so that evaluation can reconstruct their values, although
/// the solver no longer chooses those values directly.
///
/// >>> from ommx import Instance
/// >>> original = Instance.maximize()
/// >>> x = original.new_binary("x")
/// >>> y = original.new_binary("y")
/// >>> original.objective = x + y
/// >>> fixed = original.partial_evaluate({x.id: 1})
/// >>> assert {v.id for v in fixed.decision_variables} == {x.id, y.id}
/// >>> assert {v.id for v in fixed.used_decision_variables} == {y.id}
/// >>> assert fixed.evaluate({y.id: 0}).state.entries == {x.id: 1.0, y.id: 0.0}
///
/// Instances can retain information used only to interpret results:
/// {class}`~ommx.NamedFunction` expressions for reporting metrics, removed
/// constraints for checking original conditions, and an
/// {class}`~ommx.OutputObjective` separate from the current solver-facing
/// objective. A variable appearing only in this information is not thereby
/// a used variable. {meth}`~ommx.Instance.populate_state` restores fixed and
/// dependent values before evaluation. An otherwise irrelevant variable is
/// assigned a default when omitted; that default is not a reconstructed
/// candidate value. Supply its value explicitly when it matters to the output.
///
/// Reconstruction does not by itself establish feasibility or equivalence
/// between the original and transformed optimization problems. Read the
/// guarantees of each transformation, such as {meth}`~ommx.Instance.substitute`,
/// and the output semantics of {meth}`~ommx.Instance.evaluate`.
///
/// For example, changing only the active objective can preserve the objective
/// reported after evaluation, including when all solver-input variables have
/// been fixed:
///
/// >>> from ommx import DecisionVariable, Instance, Sense
/// >>> x = DecisionVariable.binary(0)
/// >>> instance = Instance.from_components(
/// ...     decision_variables=[x], objective=3 * x, constraints={},
/// ...     sense=Sense.Maximize,
/// ... )
/// >>> assert instance.convert_active_objective(Sense.Minimize)
/// >>> fixed = instance.partial_evaluate({0: 1})
/// >>> assert fixed.sense == Sense.Minimize
/// >>> assert fixed.objective.evaluate({}) == -3.0
/// >>> assert fixed.required_ids() == set()
/// >>> assert fixed.used_decision_variables == []
/// >>> assert fixed.populate_state({}).entries == {0: 1.0}
/// >>> solution = fixed.evaluate({})
/// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Instance {
    pub(crate) inner: ommx::Instance,
}

impl_instance_annotations!(Instance);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    /// Unique names across all constraint families, as a set of strings.
    ///
    /// Only active constraints are included.
    /// The scope matches `extract_constraints`. Unnamed constraints are excluded;
    /// an explicitly empty name is included. Duplicate names appear once, even
    /// when extraction would reject duplicate subscripts.
    /// Each access returns a fresh set; editing it does not change this owner.
    #[getter]
    fn constraint_names(&self) -> BTreeSet<String> {
        self.inner.constraint_names()
    }

    /// Apply one simultaneous bound-tightening pass using all active regular constraints.
    ///
    /// Calls {meth}`tighten_bounds_simultaneously_once_using_constraints` with
    /// every active regular constraint ID and the same ``max_terms`` limit.
    /// Non-affine rows and rows exceeding ``max_terms`` variable terms are skipped. All
    /// rows read the bounds at entry, and updates are applied together once.
    /// Returns a dictionary of variable IDs to their updated {class}`~ommx.Bound`.
    /// The same tolerance, supported-domain and atomicity rules apply as for
    /// the explicitly selected form.
    /// ``max_terms`` defaults to 32 and excludes the constant term.
    ///
    /// # Examples
    ///
    /// A second call can propagate a bound learned by the first call. These
    /// methods perform a single pass each, rather than iterating to a fixed point.
    ///
    /// >>> from ommx import Bound, Instance
    /// >>> instance = Instance.minimize()
    /// >>> x = instance.new_integer("x", lower=0, upper=10)
    /// >>> y = instance.new_integer("y", lower=0, upper=10)
    /// >>> _ = instance.add_constraint(x <= 2)
    /// >>> _ = instance.add_constraint(y <= x)
    /// >>> assert instance.tighten_bounds_simultaneously_once() == {x.id: Bound(0, 2)}
    /// >>> assert instance.tighten_bounds_simultaneously_once(max_terms=64) == {y.id: Bound(0, 2)}
    /// >>> assert instance.tighten_bounds_simultaneously_once() == {}
    #[pyo3(signature = (*, max_terms=32, atol=None))]
    pub fn tighten_bounds_simultaneously_once(
        &mut self,
        py: Python<'_>,
        max_terms: usize,
        atol: Option<f64>,
    ) -> OmmxPyResult<BTreeMap<u64, crate::VariableBound>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = atol.map(ommx::ATol::new).transpose()?.unwrap_or_default();
        Ok(self
            .inner
            .tighten_bounds_simultaneously_once(max_terms, atol)?
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), crate::VariableBound(bound)))
            .collect())
    }

    /// Apply one simultaneous bound-tightening pass using selected regular constraints.
    ///
    /// ``constraint_ids`` is a set of active regular constraint IDs. Unknown
    /// and removed IDs are errors; an empty set applies no updates. Selected
    /// non-affine rows are skipped. All eligible variables in the selected
    /// rows may have their bounds tightened.
    /// Only compact polynomial functions of degree at most one are used;
    /// composed expressions are skipped even when mathematically affine.
    ///
    /// ``max_terms`` limits each affine row to this many variable terms
    /// (default: 32). The constant term does not count. Rows exceeding the
    /// limit are skipped before domain lookup or candidate evaluation. Terms
    /// of fixed, semi and dependent variables still count. With a zero limit,
    /// only constant rows are processed, including contradiction detection.
    ///
    /// Returns a dictionary of variable IDs to their updated {class}`~ommx.Bound`.
    /// Every row reads the bounds at entry; all updates are collected and applied
    /// together. Newly tightened bounds are not reused during this call. Call
    /// again to propagate changes through further rows.
    /// Candidate extrema are combined before comparing changes with ``atol``.
    /// When continuous stored endpoints cross but their tolerated domains overlap,
    /// use the interval between the endpoints, intersected with the entry bounds.
    /// Both sides of equalities are used. Tolerance is accounted for algebraically:
    /// continuous domains expand to ``[lower - atol, upper + atol]``, and row
    /// residuals may be at most ``atol``. For ``a*x + r <= 0`` with ``a > 0``,
    /// the limit is ``(atol - min(r)) / a``. Subtract ``atol`` to store a continuous
    /// upper bound, or round down for an integer/binary upper bound. Negative
    /// coefficients give the corresponding lower bound. Changes within ``atol``
    /// are ignored.
    ///
    /// Residual intervals use {meth}`~ommx.Function.evaluate_bound`; candidate
    /// arithmetic uses ordinary floating-point operations. No point-evaluation
    /// boundary is searched. Rounding, cancellation and evaluation order can
    /// change feasibility near numerical boundaries, even when subsequent
    /// evaluation uses the same ``atol``.
    ///
    /// Only the selected active regular constraints are used; special
    /// constraints do not contribute. Infinite domain endpoints are allowed:
    /// an initially unbounded variable can acquire a finite bound. An upper/lower
    /// candidate with a non-finite required residual or boundary calculation is
    /// skipped; other candidates in the same row are still processed. Semi-variable domains include
    /// zero when deriving other bounds, but semi, fixed and dependent variables
    /// are not changed. This is not a complete infeasibility detector.
    ///
    /// Requires finite ``0 < atol < 1``. On failure the instance is unchanged.
    /// If omitted, {func}`~ommx.get_default_atol` supplies the tolerance.
    ///
    /// # Examples
    ///
    /// Select existing constraint IDs from a complete problem. With
    /// ``atol=0.125``, the row ``2*x <= 6`` permits ``x <= 3.0625``. The
    /// continuous bound's own tolerance is subtracted, so its stored upper
    /// endpoint becomes ``2.9375``. Row 101 is not selected here and does not
    /// impose a lower bound.
    ///
    /// >>> from ommx import Bound, ConstraintID, DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.continuous(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=0, sense=Sense.Minimize,
    /// ...     constraints={100: 2 * x <= 6, 101: x >= -2},
    /// ... )
    /// >>> changed = instance.tighten_bounds_simultaneously_once_using_constraints(
    /// ...     {ConstraintID(100)}, max_terms=64, atol=0.125,
    /// ... )
    /// >>> assert changed == {x.id: Bound(float("-inf"), 2.9375)}
    /// >>> assert instance.get_decision_variable_by_id(x.id).bound == changed[x.id]
    #[pyo3(signature = (constraint_ids, *, max_terms=32, atol=None))]
    pub(crate) fn tighten_bounds_simultaneously_once_using_constraints(
        &mut self,
        py: Python<'_>,
        constraint_ids: ConstraintSetInput<ConstraintIDInput>,
        max_terms: usize,
        atol: Option<f64>,
    ) -> OmmxPyResult<BTreeMap<u64, crate::VariableBound>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = atol.map(ommx::ATol::new).transpose()?.unwrap_or_default();
        let ids = constraint_ids.into_iter().map(ConstraintID::from).collect();
        Ok(self
            .inner
            .tighten_bounds_simultaneously_once_using_constraints(&ids, max_terms, atol)?
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), crate::VariableBound(bound)))
            .collect())
    }
}

impl Instance {
    fn empty_with_sense(sense: Sense) -> OmmxPyResult<Self> {
        Self::from_components(
            sense,
            Function(ommx::Function::Zero),
            Vec::new(),
            Default::default(),
            None,
            None,
            None,
            None,
            None,
        )
    }

    fn decision_variable_label(
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: StringMapping,
        description: Option<String>,
    ) -> ommx::DecisionVariableLabel {
        ommx::DecisionVariableLabel {
            name,
            subscripts,
            parameters: parameters.0.into_iter().collect(),
            description,
        }
    }

    fn atol_or_default(value: Option<f64>) -> OmmxPyResult<ommx::ATol> {
        match value {
            Some(value) => Ok(ommx::ATol::new(value)?),
            None => Ok(ommx::ATol::default()),
        }
    }

    fn attach_decision_variable(
        slf: Bound<'_, Self>,
        id: ommx::VariableID,
    ) -> crate::AttachedDecisionVariable {
        crate::AttachedDecisionVariable::from_instance(slf.unbind(), id)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    /// Deserialize an instance from v1 protobuf bytes.
    ///
    /// Raises {class}`ValueError` if the protobuf payload is malformed or
    /// semantically invalid.
    #[staticmethod]
    pub fn from_v1_bytes(py: Python<'_>, bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(Self {
            inner: ommx::Instance::from_v1_bytes(bytes.as_bytes())?,
        })
    }

    /// Deserialize an instance from v2 protobuf bytes.
    ///
    /// Raises {class}`ValueError` if the protobuf payload is malformed or
    /// semantically invalid.
    #[staticmethod]
    pub fn from_v2_bytes(py: Python<'_>, bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(Self {
            inner: ommx::Instance::from_v2_bytes(bytes.as_bytes())?,
        })
    }

    /// Create an instance from its components.
    ///
    /// Constraint mappings accept integer keys or IDs of the matching family.
    /// Integers are promoted to that family's ID. IDs of another family raise
    /// {class}`TypeError`; keys that become duplicates raise {class}`ValueError`.
    /// Returned collections always use typed IDs.
    ///
    /// **Args:**
    /// - `sense`: Optimization sense (minimize or maximize)
    /// - `objective`: Objective function
    /// - `decision_variables`: List of decision variables
    /// - `constraints`: Mapping from regular constraint IDs to constraints
    /// - `named_functions`: Optional list of named functions
    /// - `description`: Optional instance description
    ///
    /// **Returns:**
    /// A new Instance
    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, indicator_constraints=None, one_hot_constraints=None, sos1_constraints=None, named_functions=None, description=None))]
    pub(crate) fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: ConstraintMapInput<ConstraintIDInput, Constraint>,
        indicator_constraints: Option<
            ConstraintMapInput<IndicatorConstraintIDInput, crate::IndicatorConstraint>,
        >,
        one_hot_constraints: Option<
            ConstraintMapInput<OneHotConstraintIDInput, crate::OneHotConstraint>,
        >,
        sos1_constraints: Option<ConstraintMapInput<Sos1ConstraintIDInput, crate::Sos1Constraint>>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<InstanceDescription>,
    ) -> OmmxPyResult<Self> {
        let mut rust_decision_variables = BTreeMap::new();
        let mut variable_labels = ommx::VariableLabelStore::default();
        for var in decision_variables {
            let id = var.0;
            variable_labels.insert(id, var.2);
            if rust_decision_variables.insert(id, var.1).is_some() {
                return Err(PyValueError::new_err(format!(
                    "Duplicate decision variable ID: {}",
                    id.into_inner()
                ))
                .into());
            }
        }

        let mut constraint_context = ommx::ConstraintContextStore::default();
        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, c)| {
                let cid = ConstraintID::from(id);
                constraint_context.insert(cid, c.1);
                (cid, c.0)
            })
            .collect();

        let mut builder = ommx::Instance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .variable_labels(variable_labels)
            .constraints(rust_constraints);

        let mut indicator_context_pairs: Vec<(
            ommx::IndicatorConstraintID,
            ommx::ConstraintContext,
        )> = Vec::new();
        if let Some(ics) = indicator_constraints {
            let rust_indicator_constraints: BTreeMap<
                ommx::IndicatorConstraintID,
                ommx::IndicatorConstraint,
            > = ics
                .into_iter()
                .map(|(id, ic)| {
                    let iid = ommx::IndicatorConstraintID::from(id);
                    indicator_context_pairs.push((iid, ic.1));
                    (iid, ic.0)
                })
                .collect();
            builder = builder.indicator_constraints(rust_indicator_constraints);
        }

        let mut one_hot_context = ommx::ConstraintContextStore::default();
        if let Some(ohs) = one_hot_constraints {
            let rust_one_hot_constraints: BTreeMap<
                ommx::OneHotConstraintID,
                ommx::OneHotConstraint,
            > = ohs
                .into_iter()
                .map(|(id, oh)| {
                    let oid = ommx::OneHotConstraintID::from(id);
                    one_hot_context.insert(oid, oh.1);
                    (oid, oh.0)
                })
                .collect();
            builder = builder.one_hot_constraints(rust_one_hot_constraints);
        }

        let mut sos1_context = ommx::ConstraintContextStore::default();
        if let Some(s1s) = sos1_constraints {
            let rust_sos1_constraints: BTreeMap<ommx::Sos1ConstraintID, ommx::Sos1Constraint> = s1s
                .into_iter()
                .map(|(id, s1)| {
                    let sid = ommx::Sos1ConstraintID::from(id);
                    sos1_context.insert(sid, s1.1);
                    (sid, s1.0)
                })
                .collect();
            builder = builder.sos1_constraints(rust_sos1_constraints);
        }

        let mut named_function_labels = ommx::NamedFunctionLabelStore::default();
        if let Some(nfs) = named_functions {
            let mut rust_named_functions = BTreeMap::new();
            for nf in nfs {
                let id = nf.0;
                named_function_labels.insert(id, nf.2);
                if rust_named_functions.insert(id, nf.1).is_some() {
                    return Err(PyValueError::new_err(format!(
                        "Duplicate named function ID: {}",
                        id.into_inner()
                    ))
                    .into());
                }
            }
            builder = builder.named_functions(rust_named_functions);
        }

        if let Some(desc) = description {
            builder = builder.description(desc.0);
        }

        let mut indicator_context = ommx::ConstraintContextStore::default();
        for (id, context) in indicator_context_pairs {
            indicator_context.insert(id, context);
        }
        let inner = builder
            .constraint_context(constraint_context)
            .indicator_constraint_context(indicator_context)
            .one_hot_constraint_context(one_hot_context)
            .sos1_constraint_context(sos1_context)
            .named_function_labels(named_function_labels)
            .build()?;

        Ok(Self { inner })
    }

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables.
    ///
    /// # Examples
    ///
    /// >>> from ommx import Instance, Sense
    /// >>> instance = Instance.minimize()
    /// >>> instance.sense == Sense.Minimize
    /// True
    #[deprecated(note = "Use Instance.minimize() instead.")]
    #[staticmethod]
    pub fn empty() -> OmmxPyResult<Self> {
        Self::empty_with_sense(Sense::Minimize)
    }

    /// Create an empty minimization instance with a zero objective.
    ///
    /// Decision variables and constraints can be added incrementally with the
    /// `new_*` methods and {meth}`add_constraint`.
    #[staticmethod]
    pub fn minimize() -> OmmxPyResult<Self> {
        Self::empty_with_sense(Sense::Minimize)
    }

    /// Create an empty maximization instance with a zero objective.
    ///
    /// Decision variables and constraints can be added incrementally with the
    /// `new_*` methods and {meth}`add_constraint`.
    #[staticmethod]
    pub fn maximize() -> OmmxPyResult<Self> {
        Self::empty_with_sense(Sense::Maximize)
    }

    /// Compatibility alias for {attr}`Sense.Maximize`.
    #[classattr]
    #[pyo3(name = "MAXIMIZE")]
    fn class_maximize() -> Sense {
        Sense::Maximize
    }

    /// Compatibility alias for {attr}`Sense.Minimize`.
    #[classattr]
    #[pyo3(name = "MINIMIZE")]
    fn class_minimize() -> Sense {
        Sense::Minimize
    }

    #[gen_stub(override_return_type(type_repr = "type[InstanceDescription]"))]
    #[classattr]
    #[pyo3(name = "Description")]
    fn class_description(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.get_type::<InstanceDescription>().into_any().unbind())
    }

    /// Active optimization sense used by the solver-facing formulation.
    ///
    /// # Postconditions
    ///
    /// The property reports the active sense even when evaluation uses a distinct output sense.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> assert instance.sense == Sense.Minimize
    /// >>> assert instance.evaluate({0: 1}).sense == Sense.Maximize
    #[getter]
    pub fn sense(&self) -> Sense {
        self.inner.sense().into()
    }

    /// Active objective used by the solver-facing formulation.
    ///
    /// # Postconditions
    ///
    /// Assignment replaces the active objective and rebases subsequent output evaluation onto it.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> assert instance.objective.evaluate({0: 1}) == -1.0
    /// >>> instance.objective = 2 * x
    /// >>> solution = instance.evaluate({0: 1})
    /// >>> assert instance.sense == Sense.Minimize
    /// >>> assert (solution.sense, solution.objective) == (Sense.Minimize, 2.0)
    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.inner.objective().clone())
    }

    #[setter]
    pub fn set_objective(&mut self, objective: Function) -> OmmxPyResult<()> {
        self.inner.set_objective(objective.0)?;
        Ok(())
    }

    /// Read-only output objective used by {meth}`~ommx.Instance.evaluate` and
    /// {meth}`~ommx.Instance.evaluate_samples`, if one has been captured.
    ///
    /// A newly constructed Instance has no output objective; evaluation uses
    /// its active {attr}`~ommx.Instance.objective` and {attr}`~ommx.Instance.sense`.
    /// Transformations can capture a separate objective and sense for reporting.
    /// See {meth}`~ommx.Instance.convert_active_objective` for a complete example
    /// contrasting solver-facing and reported values. Capturing an output
    /// objective does not itself prove feasibility or optimality.
    ///
    /// # Postconditions
    ///
    /// Absence and an explicit output objective equal to the active pair remain distinct.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.output_objective is None
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> output = instance.output_objective
    /// >>> assert output is not None
    /// >>> assert output.sense == Sense.Maximize
    /// >>> assert output.function.evaluate({0: 1}) == 3.0
    /// >>> assert output.preserves_optimality
    /// >>> assert instance.convert_active_objective(Sense.Maximize)
    /// >>> output = instance.output_objective
    /// >>> assert output is not None
    /// >>> assert output.sense == instance.sense
    /// >>> assert output.function.almost_equal(instance.objective)
    #[getter]
    pub fn output_objective(&self) -> Option<OutputObjective> {
        self.inner.output_objective().cloned().map(Into::into)
    }

    /// Get all unique decision variable names in this instance
    #[getter]
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    /// Create an ID-keyed {class}`~ommx.State` from values grouped by name and subscripts.
    ///
    /// >>> from ommx import Instance, NamedState
    /// >>> instance = Instance.minimize()
    /// >>> x = instance.new_binary("x", subscripts=[0])
    /// >>> values: NamedState = {"x": {(0,): 1.0}}
    /// >>> state = instance.state_from_named_values(values)
    /// >>> state.entries == {x.id: 1.0}
    /// True
    ///
    /// Use tuple keys: `()` for scalars, `(i,)` for one-dimensional variables,
    /// and `(i, j)` for two-dimensional variables. This also accepts the output
    /// of {meth}`~ommx.Solution.extract_all_decision_variables`, resolving IDs
    /// against this instance even when the source solution used different IDs.
    ///
    /// Partial states and empty mappings are accepted. Omitted variables are
    /// not filled or inferred. Conversion does not validate values against
    /// kind, bounds, fixed values, or dependencies, or require finite values;
    /// use {meth}`~ommx.Instance.evaluate` for validation.
    ///
    /// Only names and subscripts identify variables; parameters and descriptions
    /// are ignored. Unnamed variables cannot be addressed. Raises `KeyError`
    /// for an unknown name/subscript pair and `ValueError` when a supplied pair
    /// matches multiple variables. In that case, use an ID-keyed state to
    /// distinguish them. Ambiguous labels not supplied are allowed.
    /// Input keys that repeat after string or integer normalization also raise
    /// `ValueError` instead of overwriting earlier values.
    /// Neither this instance nor the input mapping is modified.
    ///
    /// **Reuse a candidate after rebuilding a model**
    ///
    /// A rebuilt model can assign different IDs to the same labeled variables.
    /// Extract labels from the source result, then resolve them against the
    /// target. The source candidate is feasible here, but the target tightens
    /// the bound of ``y`` from 3 to 1:
    ///
    /// >>> from ommx import Instance
    /// >>> source = Instance.maximize()
    /// >>> x = [source.new_binary("x", subscripts=[i]) for i in range(2)]
    /// >>> y = source.new_integer("y", lower=0, upper=3)
    /// >>> source.objective = 3 * x[0] + 5 * x[1] + y
    /// >>> _ = source.add_constraint(x[0] + x[1] <= 1)
    /// >>> solution = source.evaluate({x[0].id: 1, x[1].id: 0, y.id: 2})
    /// >>> assert solution.feasible
    /// >>> named_values = solution.extract_all_decision_variables()
    /// >>> target = Instance.maximize()
    /// >>> target_y = target.new_integer("y", lower=0, upper=1)
    /// >>> target_x1 = target.new_binary("x", subscripts=[1])
    /// >>> target_x0 = target.new_binary("x", subscripts=[0])
    /// >>> target.objective = 3 * target_x0 + 5 * target_x1 + target_y
    /// >>> _ = target.add_constraint(target_x0 + target_x1 <= 1)
    /// >>> state = target.state_from_named_values(named_values)
    /// >>> assert state.entries == {target_x0.id: 1.0, target_x1.id: 0.0, target_y.id: 2.0}
    /// >>> assert not target.evaluate(state).feasible
    ///
    /// Matching labels transfers the values; it does not establish feasibility
    /// for the new problem. Unnamed variables and other values omitted from the
    /// extraction are not transferred. If the state is to be used as a solver's
    /// initial candidate, also consult that adapter's ``initial_state`` contract.
    fn state_from_named_values(&self, values: NamedStateInput) -> OmmxPyResult<State> {
        Ok(State(self.inner.state_from_named_values(&values.0)?))
    }

    /// Get all unique named function names in this instance
    #[getter]
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.inner.named_function_names()
    }

    /// List of all decision variables in the instance sorted by their IDs.
    ///
    /// Returns a list of {class}`~ommx.AttachedDecisionVariable` write-through
    /// handles. Each handle reads its kind / bound / label live from this
    /// instance's SoA store and writes label updates back through to it.
    /// Handles also participate in arithmetic to build expressions
    /// (`x + y`, `2 * x` etc.) — only their id is consumed for that, no host
    /// borrow is taken. Call
    /// {meth}`~ommx.AttachedDecisionVariable.detach` if you need an
    /// independent {class}`~ommx.DecisionVariable` snapshot.
    #[getter]
    pub fn decision_variables(slf: Bound<'_, Self>) -> Vec<crate::AttachedDecisionVariable> {
        let py = slf.py();
        let ids: Vec<VariableID> = slf
            .borrow()
            .inner
            .decision_variables()
            .keys()
            .copied()
            .collect();
        let py_instance: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| crate::AttachedDecisionVariable::from_instance(py_instance.clone_ref(py), id))
            .collect()
    }

    /// Add a decision variable to this instance.
    ///
    /// Drains the wrapper's modeling-label snapshot into this instance's SoA
    /// store and returns an {class}`~ommx.AttachedDecisionVariable`
    /// bound to the variable's id — a write-through handle for further
    /// label updates. The original wrapper is not modified.
    ///
    /// Raises {class}`ValueError` if the variable's id collides with an
    /// existing decision variable. Substituted variables retain their ids and
    /// therefore also count as existing decision variables.
    pub fn add_decision_variable(
        slf: Bound<'_, Self>,
        variable: DecisionVariable,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_decision_variable(variable.0, variable.1, variable.2)?
        };
        Ok(crate::AttachedDecisionVariable::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Create and add a binary decision variable with an automatically assigned ID.
    ///
    /// Returns an {class}`~ommx.AttachedDecisionVariable` that can be used
    /// directly in expressions. The numeric ID remains available through its
    /// {attr}`~ommx.AttachedDecisionVariable.id` property.
    ///
    /// **Args:**
    /// - `name`: Optional human-readable modeling name. Names need not be unique.
    /// - `subscripts`: Optional integer indices from the source model.
    /// - `parameters`: Optional string-valued indices from the source model.
    /// - `description`: Optional human-readable description.
    ///
    /// Raises {class}`ValueError` if the maximum decision-variable ID is
    /// `2**64 - 1` and no larger automatic ID can be assigned. On failure, no
    /// variable or modeling label is added to the instance.
    #[pyo3(signature = (name=None, *, subscripts=Vec::new(), parameters=StringMapping::default(), description=None))]
    fn new_binary(
        slf: Bound<'_, Self>,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: StringMapping,
        description: Option<String>,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let label = Self::decision_variable_label(name, subscripts, parameters, description);
        let id = {
            slf.borrow_mut().inner.new_binary(
                ommx::Bound::of_binary(),
                label,
                None,
                ommx::ATol::default(),
            )?
        };
        Ok(Self::attach_decision_variable(slf, id))
    }

    /// Create and add an integer decision variable with an automatically assigned ID.
    ///
    /// The bounds default to `(-inf, inf)`. Each finite side is normalized to
    /// the least or greatest integer value that satisfies the original bound
    /// under `atol`; an infinite side remains unbounded. Bound membership uses
    /// the same inequality-residual tolerance semantics as constraint
    /// feasibility. Returns an
    /// {class}`~ommx.AttachedDecisionVariable` that can be used directly in
    /// expressions.
    ///
    /// **Args:**
    /// - `name`: Optional human-readable modeling name. Names need not be unique.
    /// - `lower`: Lower bound of the variable.
    /// - `upper`: Upper bound of the variable.
    /// - `subscripts`: Optional integer indices from the source model.
    /// - `parameters`: Optional string-valued indices from the source model.
    /// - `description`: Optional human-readable description.
    /// - `atol`: Absolute tolerance for integer-bound normalization. If
    ///   omitted, the current default returned by
    ///   {func}`~ommx.get_default_atol` is used.
    ///
    /// Raises {class}`ValueError` if the bounds are invalid or normalization
    /// yields no integer value, or if no larger automatic ID can be assigned.
    /// On failure, no variable or modeling label is added to the instance.
    #[pyo3(signature = (name=None, *, lower=f64::NEG_INFINITY, upper=f64::INFINITY, subscripts=Vec::new(), parameters=StringMapping::default(), description=None, atol=None))]
    fn new_integer(
        slf: Bound<'_, Self>,
        name: Option<String>,
        lower: f64,
        upper: f64,
        subscripts: Vec<i64>,
        parameters: StringMapping,
        description: Option<String>,
        atol: Option<f64>,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let bound = ommx::Bound::new(lower, upper)?;
        let label = Self::decision_variable_label(name, subscripts, parameters, description);
        let atol = Self::atol_or_default(atol)?;
        let id = {
            slf.borrow_mut()
                .inner
                .new_integer(bound, label, None, atol)?
        };
        Ok(Self::attach_decision_variable(slf, id))
    }

    /// Create and add a continuous decision variable with an automatically assigned ID.
    ///
    /// The bounds default to `(-inf, inf)`. Returns an
    /// {class}`~ommx.AttachedDecisionVariable` that can be used directly in
    /// expressions.
    ///
    /// **Args:**
    /// - `name`: Optional human-readable modeling name. Names need not be unique.
    /// - `lower`: Lower bound of the variable.
    /// - `upper`: Upper bound of the variable.
    /// - `subscripts`: Optional integer indices from the source model.
    /// - `parameters`: Optional string-valued indices from the source model.
    /// - `description`: Optional human-readable description.
    ///
    /// Raises {class}`ValueError` if the bounds are invalid or if no larger
    /// automatic ID can be assigned. On failure, no variable or modeling label
    /// is added to the instance.
    #[pyo3(signature = (name=None, *, lower=f64::NEG_INFINITY, upper=f64::INFINITY, subscripts=Vec::new(), parameters=StringMapping::default(), description=None))]
    fn new_continuous(
        slf: Bound<'_, Self>,
        name: Option<String>,
        lower: f64,
        upper: f64,
        subscripts: Vec<i64>,
        parameters: StringMapping,
        description: Option<String>,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let bound = ommx::Bound::new(lower, upper)?;
        let label = Self::decision_variable_label(name, subscripts, parameters, description);
        let id = {
            slf.borrow_mut()
                .inner
                .new_continuous(bound, label, None, ommx::ATol::default())?
        };
        Ok(Self::attach_decision_variable(slf, id))
    }

    /// Create and add a semi-integer decision variable with an automatically assigned ID.
    ///
    /// The bounds default to `(-inf, inf)`. Each finite side is normalized under
    /// `atol` using the same residual-feasibility rule as
    /// {meth}`~ommx.Instance.new_integer`, while an infinite side remains
    /// unbounded. Unlike an integer variable, if the bound contains no integer,
    /// its normalized bound becomes `[0, 0]`, preserving the zero alternative
    /// in the semi-integer domain. Returns an
    /// {class}`~ommx.AttachedDecisionVariable` that can be used directly in
    /// expressions.
    ///
    /// **Args:**
    /// - `name`: Optional human-readable modeling name. Names need not be unique.
    /// - `lower`: Lower bound of the variable.
    /// - `upper`: Upper bound of the variable.
    /// - `subscripts`: Optional integer indices from the source model.
    /// - `parameters`: Optional string-valued indices from the source model.
    /// - `description`: Optional human-readable description.
    /// - `atol`: Absolute tolerance for integer-bound normalization. If
    ///   omitted, the current default returned by
    ///   {func}`~ommx.get_default_atol` is used.
    ///
    /// Raises {class}`ValueError` if the bounds are invalid or if no larger
    /// automatic ID can be assigned. On failure, no variable or modeling label
    /// is added to the instance.
    #[pyo3(signature = (name=None, *, lower=f64::NEG_INFINITY, upper=f64::INFINITY, subscripts=Vec::new(), parameters=StringMapping::default(), description=None, atol=None))]
    fn new_semi_integer(
        slf: Bound<'_, Self>,
        name: Option<String>,
        lower: f64,
        upper: f64,
        subscripts: Vec<i64>,
        parameters: StringMapping,
        description: Option<String>,
        atol: Option<f64>,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let bound = ommx::Bound::new(lower, upper)?;
        let label = Self::decision_variable_label(name, subscripts, parameters, description);
        let atol = Self::atol_or_default(atol)?;
        let id = {
            slf.borrow_mut()
                .inner
                .new_semi_integer(bound, label, None, atol)?
        };
        Ok(Self::attach_decision_variable(slf, id))
    }

    /// Create and add a semi-continuous decision variable with an automatically assigned ID.
    ///
    /// The bounds default to `(-inf, inf)`. Returns an
    /// {class}`~ommx.AttachedDecisionVariable` that can be used directly in
    /// expressions.
    ///
    /// **Args:**
    /// - `name`: Optional human-readable modeling name. Names need not be unique.
    /// - `lower`: Lower bound of the variable.
    /// - `upper`: Upper bound of the variable.
    /// - `subscripts`: Optional integer indices from the source model.
    /// - `parameters`: Optional string-valued indices from the source model.
    /// - `description`: Optional human-readable description.
    ///
    /// Raises {class}`ValueError` if the bounds are invalid or if no larger
    /// automatic ID can be assigned. On failure, no variable or modeling label
    /// is added to the instance.
    #[pyo3(signature = (name=None, *, lower=f64::NEG_INFINITY, upper=f64::INFINITY, subscripts=Vec::new(), parameters=StringMapping::default(), description=None))]
    fn new_semi_continuous(
        slf: Bound<'_, Self>,
        name: Option<String>,
        lower: f64,
        upper: f64,
        subscripts: Vec<i64>,
        parameters: StringMapping,
        description: Option<String>,
    ) -> OmmxPyResult<crate::AttachedDecisionVariable> {
        let bound = ommx::Bound::new(lower, upper)?;
        let label = Self::decision_variable_label(name, subscripts, parameters, description);
        let id = {
            slf.borrow_mut()
                .inner
                .new_semi_continuous(bound, label, None, ommx::ATol::default())?
        };
        Ok(Self::attach_decision_variable(slf, id))
    }

    /// Return an {class}`~ommx.AttachedDecisionVariable` bound to the
    /// given id — a write-through handle whose label setters update
    /// this instance's SoA store. The handle also participates in
    /// arithmetic via `ToFunction` (only its id is consumed). Call
    /// {meth}`~ommx.AttachedDecisionVariable.detach` to obtain an
    /// independent {class}`~ommx.DecisionVariable` snapshot.
    ///
    /// Raises {class}`KeyError` if no variable with `variable_id` exists.
    pub fn attached_decision_variable(
        slf: Bound<'_, Self>,
        variable_id: u64,
    ) -> PyResult<crate::AttachedDecisionVariable> {
        let id = VariableID::from(variable_id);
        if !slf.borrow().inner.decision_variables().contains_key(&id) {
            return Err(PyKeyError::new_err(format!(
                "Decision variable with ID {variable_id} not found"
            )));
        }
        Ok(crate::AttachedDecisionVariable::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Extract active constraints of every family by name, keyed by subscript tuples.
    ///
    /// Values are attached handles, just as in `constraints`: edits update this
    /// model. The returned dictionary snapshots the matching keys; it does not
    /// track later label or membership changes. Removed constraints are excluded.
    /// Parameters and descriptions do not participate in matching.
    ///
    /// Raises KeyError if no active constraint has this name, and ValueError if
    /// matching constraints share subscripts, including across different families.
    /// Use typed-ID lookup in `constraints` to distinguish ambiguous labels.
    fn extract_constraints(
        slf: Bound<'_, Self>,
        name: &str,
    ) -> PyResult<
        crate::subscript_map::SubscriptMap<crate::constraint_collection::ConstraintCollectionValue>,
    > {
        let py = slf.py();
        Self::constraints(slf).extract_by_name(py, name)
    }

    /// Live mapping of all active constraint families, keyed by typed IDs.
    /// The view retains this owner and fetches each constraint on demand.
    #[getter]
    pub fn constraints(slf: Bound<'_, Self>) -> crate::constraint_collection::ConstraintCollection {
        crate::constraint_collection::ConstraintCollection::new(crate::ConstraintHost::Instance(
            slf.unbind(),
        ))
    }

    /// Add a regular constraint to this instance.
    ///
    /// Picks an unused {class}`~ommx.ConstraintID`, drains the wrapper's
    /// context snapshot into this instance's SoA store, and returns an
    /// {class}`~ommx.AttachedConstraint` bound to the new id. The input
    /// {class}`~ommx.Constraint` is not mutated; subsequent writes that
    /// should land in the instance must go through the returned handle.
    /// When modeling-label fields are provided, they replace the corresponding
    /// fields stored on the inserted constraint without modifying the input
    /// snapshot. Omitted fields preserve the snapshot's existing values.
    ///
    /// **Args:**
    /// - `constraint`: Constraint to add
    /// - `name`: Optional modeling name for the inserted constraint
    /// - `subscripts`: Optional integer indices for the inserted constraint
    /// - `parameters`: Optional string-valued indices for the inserted constraint
    /// - `description`: Optional description for the inserted constraint
    ///
    /// Raises {class}`ValueError` if the constraint references an undefined
    /// decision variable or one currently used as a substitution-dependency
    /// key, matching the validation performed by other constraint-insertion
    /// paths.
    #[pyo3(signature = (constraint, name=None, *, subscripts=None, parameters=None, description=None))]
    pub fn add_constraint(
        slf: Bound<'_, Self>,
        mut constraint: Constraint,
        name: Option<String>,
        subscripts: Option<Vec<i64>>,
        parameters: Option<HashMap<String, String>>,
        description: Option<String>,
    ) -> OmmxPyResult<crate::AttachedConstraint> {
        if let Some(name) = name {
            constraint.1.label.name = Some(name);
        }
        if let Some(subscripts) = subscripts {
            constraint.1.label.subscripts = subscripts;
        }
        if let Some(parameters) = parameters {
            constraint.1.label.parameters = parameters.into_iter().collect();
        }
        if let Some(description) = description {
            constraint.1.label.description = Some(description);
        }
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedConstraint::from_instance(slf.unbind(), id))
    }

    /// Add an indicator constraint to this instance.
    ///
    /// Picks an unused {class}`~ommx.IndicatorConstraintID`, drains the
    /// wrapper's context snapshot into this instance's SoA store, and
    /// returns an {class}`~ommx.AttachedIndicatorConstraint` bound to the
    /// new id.
    ///
    /// Raises {class}`ValueError` if the constraint references an undefined
    /// decision variable or one currently used as a substitution-dependency
    /// key.
    pub fn add_indicator_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::IndicatorConstraint,
    ) -> OmmxPyResult<crate::AttachedIndicatorConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_indicator_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedIndicatorConstraint::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Add a one-hot constraint to this instance.
    pub fn add_one_hot_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::OneHotConstraint,
    ) -> OmmxPyResult<crate::AttachedOneHotConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_one_hot_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedOneHotConstraint::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// Add a SOS1 constraint to this instance.
    pub fn add_sos1_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::Sos1Constraint,
    ) -> OmmxPyResult<crate::AttachedSos1Constraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_sos1_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedSos1Constraint::from_instance(
            slf.unbind(),
            id,
        ))
    }

    /// The kinds of active special constraints this instance currently uses.
    ///
    /// Returns the set of {class}`~ommx.SpecialConstraintKind` values corresponding
    /// to non-empty active (non-removed) special constraint collections. An
    /// empty set means the instance has no active special constraints.
    #[getter]
    pub fn active_special_constraint_kinds(
        &self,
    ) -> std::collections::HashSet<crate::SpecialConstraintKind> {
        self.inner
            .active_special_constraint_kinds()
            .into_iter()
            .map(|kind| kind.into())
            .collect()
    }

    /// Lower selected active special constraint kinds into regular constraints.
    ///
    /// For every kind in ``kinds_to_lower``, the corresponding bulk conversion
    /// is invoked
    /// ({meth}`~ommx.Instance.convert_all_indicators_to_constraints`,
    /// {meth}`~ommx.Instance.convert_all_one_hots_to_constraints`, or
    /// {meth}`~ommx.Instance.convert_all_sos1_to_constraints`) when that kind is active. The
    /// instance is mutated in place. Kinds omitted from ``kinds_to_lower``
    /// remain active, and an empty set is a no-op. This does not establish
    /// {class}`~ommx.InstanceClass` membership; check the resulting input separately.
    ///
    /// Returns the set of {class}`~ommx.SpecialConstraintKind` values that were
    /// requested and active, and therefore actually lowered. Empty when no
    /// requested kind was active.
    ///
    /// ``atol`` controls zero-sensitive interval bounds used while lowering
    /// Indicator constraints. If omitted, :attr:`DEFAULT_ATOL` is used. This
    /// aligns zero-sensitive Function body evaluation; lowering itself assumes
    /// exact discrete variable values.
    ///
    /// Kinds are processed in ``Indicator``, ``OneHot``, ``Sos1`` order. The
    /// Indicator and SOS1 bulk conversions validate their complete families
    /// before mutation, including generated-ID capacity. OneHot checks ID
    /// capacity for the whole family before converting in ascending original
    /// ID order; other conversion failures may leave earlier OneHot conversions
    /// committed. The whole operation is not transactional across
    /// families: an error does not roll back earlier completed families.
    ///
    /// Raises if an underlying conversion fails, including insufficient fresh
    /// IDs or a required non-finite Big-M bound.
    #[pyo3(signature = (kinds_to_lower, *, atol=None))]
    pub fn lower_special_constraints(
        &mut self,
        py: Python<'_>,
        kinds_to_lower: std::collections::HashSet<crate::SpecialConstraintKind>,
        atol: Option<f64>,
    ) -> OmmxPyResult<std::collections::HashSet<crate::SpecialConstraintKind>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let rust_kinds_to_lower: ommx::SpecialConstraintKinds =
            kinds_to_lower.into_iter().map(|kind| kind.into()).collect();
        let lowered = self
            .inner
            .lower_special_constraints(&rust_kinds_to_lower, atol)?;
        Ok(lowered.into_iter().map(|kind| kind.into()).collect())
    }

    #[doc = include_str!("docstrings/ommx/Instance.removed_constraints.md")]
    #[getter]
    pub fn removed_constraints(
        slf: Bound<'_, Self>,
    ) -> crate::constraint_collection::RemovedConstraintCollection {
        crate::constraint_collection::RemovedConstraintCollection::new(
            crate::ConstraintHost::Instance(slf.unbind()),
        )
    }

    /// List of all named functions in the instance sorted by their IDs.
    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        let labels = self.inner.named_function_labels();
        self.inner
            .named_functions()
            .iter()
            .map(|(id, named_function)| {
                NamedFunction(*id, named_function.clone(), labels.collect_for(*id))
            })
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<InstanceDescription> {
        self.inner.description.clone().map(InstanceDescription)
    }

    /// Decision variables used by the mathematical solver input.
    ///
    /// This list contains the variables referenced by the objective or by an
    /// active regular, Indicator, OneHot, or SOS1 constraint. Variables
    /// referenced only by the output objective, named functions, removed
    /// constraints, or the right-hand side of a decision-variable dependency
    /// are excluded. Fixed, dependent, and irrelevant variables are also
    /// excluded; {meth}`~ommx.Instance.evaluate` and
    /// {meth}`~ommx.Instance.evaluate_samples` populate their output values from
    /// stored values, dependencies, or defaults respectively.
    /// See {meth}`~ommx.Instance.populate_state` for the complete reconstruction
    /// and default rules and examples of retained versus used variables.
    #[getter]
    pub fn used_decision_variables(&self) -> Vec<DecisionVariable> {
        let labels = self.inner.variable_labels();
        self.inner
            .used_decision_variables()
            .iter()
            .map(|(id, &var)| {
                DecisionVariable::from_parts(*id, var.clone(), labels.collect_for(*id))
            })
            .collect()
    }

    /// Serialize this instance in the OMMX v1 wire format.
    ///
    /// # Errors
    ///
    /// Serialization raises ``RuntimeError`` whenever an output objective is present because v1 cannot represent it.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> assert instance.convert_active_objective(Sense.Maximize)
    /// >>> try:
    /// ...     instance.to_v1_bytes()
    /// ... except RuntimeError:
    /// ...     pass
    /// ... else:
    /// ...     raise AssertionError("v1 serialization accepted an output objective")
    pub fn to_v1_bytes<'py>(&self, py: Python<'py>) -> OmmxPyResult<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyBytes::new(py, &self.inner.to_v1_bytes()?))
    }

    /// Serialize this instance in the OMMX v2 wire format.
    ///
    /// # Postconditions
    ///
    /// A v2 round-trip preserves both active and output objective semantics.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> restored = Instance.from_v2_bytes(instance.to_v2_bytes())
    /// >>> assert restored.sense == Sense.Minimize
    /// >>> assert restored.objective.evaluate({0: 1}) == -3.0
    /// >>> solution = restored.evaluate({0: 1})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)
    pub fn to_v2_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        PyBytes::new(py, &self.inner.to_v2_bytes())
    }

    pub fn __str__(&self) -> String {
        self.inner.format_summary()
    }

    pub fn __repr__(&self) -> String {
        self.inner.format_summary()
    }

    /// Format a function using this instance's decision-variable labels.
    ///
    /// The plain {func}`repr` / {func}`str` representation of {class}`~ommx.Function`
    /// remains context-free and renders raw ``x<id>`` symbols such as ``x1``.
    /// This method resolves names and subscripts from this instance without
    /// changing the expression or its value.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.minimize()
    /// >>> x0 = instance.new_binary("x", subscripts=[0])
    /// >>> x1 = instance.new_binary("x", subscripts=[1])
    /// >>> instance.objective = x0 + 2 * x1
    /// >>> assert instance.format_function(instance.objective) == "x[0] + 2*x[1]"
    ///
    /// Every ID referenced by ``function`` must exist in this instance. IDs are
    /// checked before truncation, even if an unknown ID would be outside the
    /// preview. An expression does not remember the model its variables came
    /// from: using another instance resolves the same numeric IDs in that
    /// instance, not matching names across models.
    ///
    /// ``max_terms`` limits complete stored terms and ``max_chars`` limits
    /// Unicode characters; both default to ``None`` for no limit. Explicit
    /// limits can produce incomplete text. Use
    /// {meth}`~ommx.Instance.display_function` when truncation metadata or a
    /// Notebook presentation is needed. The corresponding
    /// {meth}`~ommx.ParametricInstance.format_function` also resolves parameters.
    #[pyo3(signature = (function, max_terms=None, max_chars=None))]
    pub fn format_function(
        &self,
        function: Function,
        max_terms: Option<usize>,
        max_chars: Option<usize>,
    ) -> OmmxPyResult<String> {
        let opts = ommx::FunctionFormatOptions {
            max_terms,
            max_chars,
        };
        if opts == ommx::FunctionFormatOptions::default() {
            Ok(self.inner.format_function(&function.0)?)
        } else {
            Ok(self.inner.format_function_with(&function.0, opts)?.text)
        }
    }

    /// Build a bounded notebook display object for a function in this instance's context.
    ///
    /// By default this returns a preview capped at 100 complete terms and
    /// 20,000 characters. Pass ``None`` to disable either limit. Use
    /// {meth}`~ommx.Instance.format_function` for an unbounded plain text string.
    /// ID resolution and validation are the same as for that method.
    ///
    /// The returned {class}`~ommx.display.FunctionDisplay` renders itself in a
    /// Notebook. Its ``text`` is the formatted preview; ``truncated``,
    /// ``omitted_terms``, and ``truncated_by_chars`` distinguish a preview from
    /// the complete expression. A character limit can cut through a term, so
    /// ``written_terms`` counts only terms written completely.
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.minimize()
    /// >>> x = [instance.new_binary("x", subscripts=[i]) for i in range(3)]
    /// >>> preview = instance.display_function(sum(x), max_terms=2)
    /// >>> assert preview.text == "x[0] + x[1]"
    /// >>> assert preview.truncated
    /// >>> assert (preview.total_terms, preview.written_terms, preview.omitted_terms) == (3, 2, 1)
    /// >>> complete = instance.display_function(sum(x), max_terms=None, max_chars=None)
    /// >>> assert not complete.truncated
    #[pyo3(signature = (function, max_terms=Some(100), max_chars=Some(20000)))]
    pub fn display_function(
        &self,
        function: Function,
        max_terms: Option<usize>,
        max_chars: Option<usize>,
    ) -> OmmxPyResult<crate::display::FunctionDisplay> {
        let formatted = self.inner.format_function_with(
            &function.0,
            ommx::FunctionFormatOptions {
                max_terms,
                max_chars,
            },
        )?;
        Ok(crate::display::FunctionDisplay::new(formatted))
    }

    /// Get the decision variable IDs required by the active formulation.
    ///
    /// # Postconditions
    ///
    /// IDs referenced only by preserved output semantics are not required solver inputs.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> fixed = instance.partial_evaluate({0: 1})
    /// >>> assert fixed.required_ids() == set()
    /// >>> assert fixed.evaluate({}).objective == 1.0
    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.inner
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Return the active objective in QUBO format without preparing the instance.
    ///
    /// Returns ``(Q, offset)``, where ``Q`` is a ``dict[tuple[int, int], float]``
    /// keyed by decision-variable IDs. For binary values, the formatted energy is
    /// ``offset + sum(c * state[i] * state[j] for (i, j), c in Q.items())``.
    /// Keys satisfy ``i <= j``; linear terms use ``(i, i)`` and each off-diagonal
    /// term occurs once, with no factor of two. The constant is returned separately.
    /// Missing keys have coefficient zero; unused variables need not appear.
    ///
    /// # Applicability and errors
    ///
    /// Raises ``RuntimeError`` unless all of the following hold:
    ///
    /// - The active sense is {attr}`~ommx.Sense.Minimize`.
    /// - There are no active regular, Indicator, OneHot, or SOS1 constraints.
    ///   Removed constraints do not prevent formatting and are not exported.
    /// - Every variable referenced by the active objective has Binary kind.
    ///   Other variables retained by the instance do not prevent formatting.
    /// - The active objective is stored as an expanded polynomial. Composed
    ///   expressions, including an unexpanded {meth}`~ommx.Function.powi`, are
    ///   rejected even if mathematically polynomial.
    /// - Each nonzero nonconstant term has
    ///   at most two distinct variable IDs after applying the binary identity
    ///   ``x**n = x`` for positive integer powers. Terms with the same resulting
    ///   pair are summed. A term with three distinct IDs is rejected;
    ///   terms are not reduced using auxiliary variables.
    ///
    /// This method does not mutate the instance, encode variables, add penalties,
    /// or change the sense. Use {meth}`~ommx.Instance.prepare` or
    /// {meth}`~ommx.Instance.to_qubo` when preparation is needed.
    /// {meth}`~ommx.InstanceClass.qubo` checks the stored polynomial degree
    /// (at most two), without this formatter's binary-power reduction.
    /// For example, an expanded ``x * x * x`` for Binary ``x`` can be
    /// formatted here even though it is outside that class. Membership checking,
    /// preparation, and coefficient formatting are separate operations.
    ///
    /// # Numerical behavior
    ///
    /// Coefficients and the offset use double-precision floating-point arithmetic;
    /// aggregation can round or overflow, and resulting sums are not checked for
    /// finiteness. Coefficients are not rescaled or pruned by magnitude: finite
    /// nonzero coefficients and constant offsets are retained even at or below
    /// ``2**-52`` (approximately ``2.22e-16``). Terms with the same binary key
    /// are summed, and only exact zero entries are omitted. Nonzero results of
    /// that summation are retained regardless of magnitude. Evaluation
    /// tolerances do not affect formatting.
    ///
    /// # Active and output objectives
    ///
    /// The coefficients describe the active objective. They do not carry
    /// {attr}`~ommx.Instance.output_objective`,
    /// removed constraints, or value-recovery rules. Keep the instance and use
    /// {meth}`~ommx.Instance.evaluate` or {meth}`~ommx.Instance.evaluate_samples`
    /// to interpret candidates with its output semantics. Backend energy can
    /// differ from the objective reported by those methods.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x + 5, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> qubo, offset = instance.as_qubo_format()
    /// >>> assert (qubo, offset) == ({(0, 0): -3.0}, -5.0)
    /// >>> assert instance.objective.evaluate({0: 1}) == -8.0
    /// >>> assert instance.evaluate({0: 1}).objective == 8.0
    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> OmmxPyResult<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (qubo, constant) = self.inner.as_qubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
    }

    /// Return the active objective in HUBO format without preparing the instance.
    ///
    /// Coefficients and the offset use double-precision floating-point arithmetic.
    /// Finite nonzero coefficients and constant offsets are retained regardless
    /// of magnitude, including values at or below ``2**-52``. Terms with the
    /// same binary key are summed, and only exact zero entries are omitted.
    /// Aggregation can round or overflow, and resulting sums are not checked for
    /// finiteness. Coefficients are not rescaled, and evaluation tolerances do
    /// not affect formatting. This method does not mutate the instance.
    ///
    /// # Postconditions
    ///
    /// The returned coefficients represent the active objective rather than preserved output semantics.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x + 5, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> hubo, offset = instance.as_hubo_format()
    /// >>> assert (hubo, offset) == ({(0,): -3.0}, -5.0)
    /// >>> assert instance.objective.evaluate({0: 1}) == -8.0
    /// >>> assert instance.evaluate({0: 1}).objective == 8.0
    pub fn as_hubo_format<'py>(&self, py: Python<'py>) -> OmmxPyResult<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (hubo, constant) = self.inner.as_hubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &hubo)?.extract()?, constant))
    }

    /// Convert the instance to a QUBO format.
    ///
    /// # Postconditions
    ///
    /// The driver is equivalent to QUBO Preparation followed by active-objective formatting and preserves the output semantics present on entry.
    ///
    /// Formatting follows the numerical contract of {meth}`~ommx.Instance.as_qubo_format`,
    /// including preservation of nonzero coefficients regardless of magnitude.
    ///
    /// >>> import copy
    /// >>> from ommx import DecisionVariable, Instance, InstanceClass, PreparationPolicy, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Maximize
    /// ... )
    /// >>> explicit = copy.copy(instance)
    /// >>> policy = PreparationPolicy.for_qubo(uniform_penalty_weight=2.0)
    /// >>> _ = explicit.prepare(InstanceClass.qubo(), policy)
    /// >>> expected = explicit.as_qubo_format()
    /// >>> actual = instance.to_qubo(uniform_penalty_weight=2.0)
    /// >>> assert actual == expected
    /// >>> assert InstanceClass.qubo().contains(instance)
    /// >>> assert instance.sense == Sense.Minimize
    /// >>> assert instance.objective.evaluate({0: 0}) == 2.0
    /// >>> solution = instance.evaluate({0: 0})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 0.0)
    ///
    /// # Errors
    ///
    /// Mutually exclusive penalty options raise ``ValueError`` before mutating the instance.
    ///
    /// >>> unchanged = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Maximize
    /// ... )
    /// >>> before = unchanged.to_v2_bytes()
    /// >>> try:
    /// ...     unchanged.to_qubo(uniform_penalty_weight=1.0, penalty_weights={7: 2.0})
    /// ... except ValueError:
    /// ...     pass
    /// ... else:
    /// ...     raise AssertionError("mutually exclusive penalty options were accepted")
    /// >>> assert unchanged.to_v2_bytes() == before
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_qubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> OmmxPyResult<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let policy = crate::PreparationPolicy::for_qubo(
            uniform_penalty_weight,
            penalty_weights.map(|weights| weights.into_iter().collect()),
            inequality_integer_slack_max_range,
        )?;
        self.prepare(py, &crate::InstanceClass::qubo(), &policy)?;
        self.as_qubo_format(py)
    }

    /// Convert the instance to a HUBO format.
    ///
    /// # Postconditions
    ///
    /// The driver is equivalent to HUBO Preparation followed by active-objective formatting and preserves the output semantics present on entry.
    ///
    /// Formatting follows the numerical contract of {meth}`~ommx.Instance.as_hubo_format`,
    /// including preservation of nonzero coefficients regardless of magnitude.
    ///
    /// >>> import copy
    /// >>> from ommx import DecisionVariable, Instance, InstanceClass, PreparationPolicy, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Maximize
    /// ... )
    /// >>> explicit = copy.copy(instance)
    /// >>> policy = PreparationPolicy.for_hubo(uniform_penalty_weight=2.0)
    /// >>> _ = explicit.prepare(InstanceClass.hubo(), policy)
    /// >>> expected = explicit.as_hubo_format()
    /// >>> actual = instance.to_hubo(uniform_penalty_weight=2.0)
    /// >>> assert actual == expected
    /// >>> assert InstanceClass.hubo().contains(instance)
    /// >>> assert instance.sense == Sense.Minimize
    /// >>> assert instance.objective.evaluate({0: 0}) == 2.0
    /// >>> solution = instance.evaluate({0: 0})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 0.0)
    ///
    /// # Errors
    ///
    /// Mutually exclusive penalty options raise ``ValueError`` before mutating the instance.
    ///
    /// >>> unchanged = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Maximize
    /// ... )
    /// >>> before = unchanged.to_v2_bytes()
    /// >>> try:
    /// ...     unchanged.to_hubo(uniform_penalty_weight=1.0, penalty_weights={7: 2.0})
    /// ... except ValueError:
    /// ...     pass
    /// ... else:
    /// ...     raise AssertionError("mutually exclusive penalty options were accepted")
    /// >>> assert unchanged.to_v2_bytes() == before
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_hubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> OmmxPyResult<(Bound<'py, PyDict>, f64)> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let policy = crate::PreparationPolicy::for_hubo(
            uniform_penalty_weight,
            penalty_weights.map(|weights| weights.into_iter().collect()),
            inequality_integer_slack_max_range,
        )?;
        self.prepare(py, &crate::InstanceClass::hubo(), &policy)?;
        self.as_hubo_format(py)
    }

    /// Convert this instance into a parameter-free parametric instance.
    ///
    /// # Postconditions
    ///
    /// Materializing the result without parameters preserves both active and output objective semantics.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> restored = instance.as_parametric_instance().with_parameters({})
    /// >>> assert restored.sense == Sense.Minimize
    /// >>> assert restored.objective.evaluate({0: 1}) == -1.0
    /// >>> solution = restored.evaluate({0: 1})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 1.0)
    pub fn as_parametric_instance(&self) -> ParametricInstance {
        ParametricInstance {
            inner: self.inner.clone().into(),
        }
    }

    /// Convert to a parametric unconstrained instance by penalty method.
    ///
    /// Roughly, this converts a constrained problem:
    ///
    /// $$\min_x f(x) \quad \text{s.t.} \quad g_i(x) = 0 \; (\forall i), \quad h_j(x) \leq 0 \; (\forall j)$$
    ///
    /// to an unconstrained problem with parameters:
    ///
    /// $$\min_x f(x) + \sum_i \lambda_i g_i(x)^2 + \sum_j \rho_j h_j(x)^2$$
    ///
    /// where $\lambda_i$ and $\rho_j$ are the penalty weight parameters for each constraint.
    /// If you want to use single weight parameter, use {meth}`~ommx.Instance.uniform_penalty_method` instead.
    ///
    /// The removed constraints are stored in {attr}`~ommx.ParametricInstance.removed_constraints`.
    ///
    /// > Note: This method converts inequality constraints $h(x) \leq 0$ to $|h(x)|^2$ not to $\max(0, h(x))^2$.
    /// > This means the penalty is enforced even for $h(x) < 0$ cases, and $h(x) = 0$ is unfairly favored.
    /// > This feature is intended to use with {meth}`~ommx.Instance.add_integer_slack_to_inequality`.
    ///
    /// # Postconditions
    ///
    /// Penalty conversion preserves existing output semantics, or captures the pre-penalty active objective when no output objective exists; materialization evaluates the penalty energy actively and invalidates optimality transport.
    ///
    /// >>> from ommx import ConstraintID
    /// >>> from ommx import DecisionVariable, Instance, Optimality, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Minimize
    /// ... )
    /// >>> parametric = instance.penalty_method()
    /// >>> parameters = {parameter.id: 2.0 for parameter in parametric.parameters}
    /// >>> prepared = parametric.with_parameters(parameters)
    /// >>> assert parametric.constraints == {}
    /// >>> assert ConstraintID(7) in parametric.removed_constraints
    /// >>> assert prepared.objective.evaluate({0: 0}) == 2.0
    /// >>> solution = prepared.evaluate({0: 0})
    /// >>> assert (solution.sense, solution.objective, solution.feasible) == (Sense.Minimize, 0.0, False)
    /// >>> assert prepared.map_active_optimality(Optimality.Optimal) == Optimality.Unspecified
    pub fn penalty_method(&self, py: Python<'_>) -> OmmxPyResult<ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let parametric_instance = self.inner.clone().penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
        })
    }

    /// Convert to a parametric unconstrained instance by penalty method with uniform weight.
    ///
    /// Roughly, this converts a constrained problem:
    ///
    /// $$\min_x f(x) \quad \text{s.t.} \quad g_i(x) = 0 \; (\forall i), \quad h_j(x) \leq 0 \; (\forall j)$$
    ///
    /// to an unconstrained problem with a parameter:
    ///
    /// $$\min_x f(x) + \lambda \left( \sum_i g_i(x)^2 + \sum_j h_j(x)^2 \right)$$
    ///
    /// where $\lambda$ is the uniform penalty weight parameter for all constraints.
    ///
    /// The removed constraints are stored in {attr}`~ommx.ParametricInstance.removed_constraints`.
    ///
    /// > Note: This method converts inequality constraints $h(x) \leq 0$ to $|h(x)|^2$ not to $\max(0, h(x))^2$.
    /// > This means the penalty is enforced even for $h(x) < 0$ cases, and $h(x) = 0$ is unfairly favored.
    /// > This feature is intended to use with {meth}`~ommx.Instance.add_integer_slack_to_inequality`.
    ///
    /// # Postconditions
    ///
    /// Uniform-penalty conversion preserves existing output semantics, or captures the pre-penalty active objective when no output objective exists; materialization evaluates the penalty energy actively and invalidates optimality transport.
    ///
    /// >>> from ommx import ConstraintID
    /// >>> from ommx import DecisionVariable, Instance, Optimality, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Minimize
    /// ... )
    /// >>> parametric = instance.uniform_penalty_method()
    /// >>> parameter_id = parametric.parameters[0].id
    /// >>> prepared = parametric.with_parameters({parameter_id: 2.0})
    /// >>> assert parametric.constraints == {}
    /// >>> assert ConstraintID(7) in parametric.removed_constraints
    /// >>> assert prepared.objective.evaluate({0: 0}) == 2.0
    /// >>> solution = prepared.evaluate({0: 0})
    /// >>> assert (solution.sense, solution.objective, solution.feasible) == (Sense.Minimize, 0.0, False)
    /// >>> assert prepared.map_active_optimality(Optimality.Optimal) == Optimality.Unspecified
    pub fn uniform_penalty_method(&self, py: Python<'_>) -> OmmxPyResult<ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let parametric_instance = self.inner.clone().uniform_penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
        })
    }

    /// Evaluate the given {class}`~ommx.State` into a {class}`~ommx.Solution`.
    ///
    /// Accepts a State, an ID-to-value mapping, or an iterable of ID/value pairs.
    /// This checks one candidate against this problem's variable domains and
    /// constraints and computes its output objective; it does not search for a
    /// better candidate or prove optimality. An infeasible candidate can still
    /// have an objective value. See {class}`~ommx.Solution` for a complete
    /// example and how to interpret the results.
    ///
    /// Evaluate a solver's returned candidate with the transformed Instance
    /// used for that solve. Its recorded dependencies, removed constraints,
    /// and output objective are needed to reconstruct and interpret the result.
    ///
    /// # Postconditions
    ///
    /// Evaluation first applies the canonicalization, population, and consistency
    /// assertion rules documented by {meth}`~ommx.Instance.populate_state`, then
    /// applies preserved output objective semantics to the populated state.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> fixed = instance.partial_evaluate({0: 1})
    /// >>> solution = fixed.evaluate({})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)
    ///
    /// # Errors
    ///
    /// Evaluation raises ``ValueError`` when an active required ID is missing.
    ///
    /// >>> required = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Minimize
    /// ... )
    /// >>> try:
    /// ...     required.evaluate({})
    /// ... except ValueError as error:
    /// ...     assert "missing required variable IDs" in str(error)
    /// ... else:
    /// ...     raise AssertionError("evaluation accepted a missing active ID")
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(
        &self,
        py: Python<'_>,
        state: State,
        atol: Option<f64>,
    ) -> OmmxPyResult<Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let solution = self.inner.evaluate(&state.0, atol)?;
        Ok(Solution { inner: solution })
    }

    #[doc = include_str!("docstrings/ommx/Instance.populate_state.md")]
    #[pyo3(signature = (state, *, atol=None))]
    pub fn populate_state(
        &self,
        py: Python<'_>,
        state: State,
        atol: Option<f64>,
    ) -> OmmxPyResult<State> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let state = self.inner.populate_state(state.0, atol)?;
        Ok(State(state))
    }

    /// Creates a new instance by fixing decision variables from a supplied state.
    ///
    /// This method validates supplied values, selects their Instance-owned representations
    /// under the rules below, and substitutes the specified decision variables. This creates
    /// a new problem instance where these variables are fixed and is useful for scenarios such as:
    ///
    /// - Creating simplified sub-problems with some variables fixed
    /// - Incrementally solving a problem by fixing some variables and optimizing the rest
    /// - Testing specific configurations of a problem
    ///
    /// **Args:**
    /// - `state`: Maps decision variable IDs to their fixed values.
    ///   Can be a {class}`~ommx.State` object or a dictionary mapping variable IDs to values.
    /// - `atol`: Absolute tolerance for floating point comparisons. If None, uses the default tolerance.
    ///
    /// **Returns:**
    /// A new instance with the specified decision variables fixed to values selected by the
    /// canonicalization and ownership rules below.
    ///
    /// # Postconditions
    ///
    /// After the existing kind and bound validation, accepted supplied coordinates
    /// use the same Instance-owned canonicalization rules as
    /// {meth}`~ommx.Instance.populate_state` before propagation and substitution.
    /// Non-fixed, non-dependent Continuous and SemiContinuous values are not rounded.
    /// Existing fixed values remain authoritative, dependent inputs remain consistency
    /// assertions, and values outside the existing partial-evaluation acceptance contract
    /// remain rejected. The new instance rewrites only active expressions while retaining
    /// canonical fixed values for output evaluation.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> fixed = instance.partial_evaluate({0: 1.0000005}, atol=1e-6)
    /// >>> assert instance.required_ids() == {0}
    /// >>> assert fixed.required_ids() == set()
    /// >>> assert fixed.objective.evaluate({}) == -3.0
    /// >>> assert fixed.attached_decision_variable(0).substituted_value == 1.0
    /// >>> solution = fixed.evaluate({})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(
        &self,
        py: Python<'_>,
        state: State,
        atol: Option<f64>,
    ) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let mut new_inner = self.inner.clone();
        new_inner.partial_evaluate(&state.0, atol)?;
        Ok(Self { inner: new_inner })
    }

    /// Evaluate multiple candidates into a {class}`~ommx.SampleSet`.
    ///
    /// Accepts {class}`~ommx.Samples`, a mapping from sample IDs to states, or
    /// a sequence of states with automatically assigned sample IDs. Each state
    /// maps decision-variable IDs to values. This evaluates supplied candidates;
    /// it does not generate them or establish global optimality. See
    /// {class}`~ommx.SampleSet` for a complete example, summary tables, and
    /// selection of the best feasible candidate obtained.
    ///
    /// Use the transformed Instance associated with the returned candidates,
    /// as for {meth}`~ommx.Instance.evaluate` on a single candidate.
    ///
    /// # Postconditions
    ///
    /// Every sample uses the canonicalization, population, and consistency assertion
    /// rules documented by {meth}`~ommx.Instance.populate_state` before applying
    /// preserved output objective semantics. SampleID membership is preserved.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> fixed = instance.partial_evaluate({0: 1})
    /// >>> sample_set = fixed.evaluate_samples({7: {}})
    /// >>> assert sample_set.sense == Sense.Maximize
    /// >>> assert sample_set.objectives[7] == 3.0
    /// >>> assert sample_set.get(7).state.entries == {0: 1.0}
    #[pyo3(signature = (samples, *, atol=None))]
    pub fn evaluate_samples(
        &self,
        py: Python<'_>,
        samples: Samples,
        atol: Option<f64>,
    ) -> OmmxPyResult<SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let inner = self.inner.evaluate_samples(&samples.0, atol)?;
        Ok(SampleSet { inner })
    }

    /// Generate a random state for this instance using the provided random number generator.
    ///
    /// This method generates random values only for variables that are actually used in the
    /// objective function or constraints, as determined by decision variable usage.
    /// Generated values respect the bounds of each variable type.
    ///
    /// **Args:**
    /// - `rng`: Random number generator to use for generating the state.
    ///
    /// **Returns:**
    /// A randomly generated state that satisfies the variable bounds of this instance.
    /// Only contains values for variables that are used in the problem.
    ///
    /// # Examples
    ///
    /// Generate random state only for used variables
    ///
    /// >>> from ommx import DecisionVariable, Instance, Rng, Sense
    /// >>> x = [DecisionVariable.binary(i) for i in range(5)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints={},
    /// ...     sense=Sense.Maximize,
    /// ... )
    ///
    /// >>> rng = Rng()
    /// >>> state = instance.random_state(rng)
    ///
    /// Only used variables have values
    ///
    /// >>> set(state.entries.keys())
    /// {0, 1}
    ///
    /// Values respect binary bounds
    ///
    /// >>> all(state.entries[i] in [0.0, 1.0] for i in state.entries)
    /// True
    pub fn random_state(&self, rng: &Rng) -> OmmxPyResult<crate::State> {
        let strategy = self.inner.arbitrary_state();
        let mut rng_guard = rng.lock()?;
        let state = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::State(state))
    }

    /// Generate random samples for this instance.
    ///
    /// The generated samples will contain ``num_samples`` sample entries divided into
    /// ``num_different_samples`` groups, where each group shares the same state but has
    /// different sample IDs.
    ///
    /// **Args:**
    /// - `rng`: Random number generator
    /// - `num_different_samples`: Number of different states to generate
    /// - `num_samples`: Total number of samples to generate
    /// - `max_sample_id`: Maximum sample ID (default: ``num_samples``)
    ///
    /// **Returns:**
    /// Samples object
    ///
    /// Raises {class}`ValueError` if the requested state groups cannot
    /// partition the samples or the inclusive sample-ID range is too small.
    /// `num_different_samples=0` is valid only when `num_samples=0`.
    ///
    /// # Examples
    ///
    /// Generate samples for a simple instance:
    ///
    /// >>> from ommx import DecisionVariable, Instance, Rng, Sense
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={0: sum(x) <= 2},
    /// ...     sense=Sense.Maximize,
    /// ... )
    ///
    /// >>> rng = Rng()
    /// >>> samples = instance.random_samples(rng, num_different_samples=2, num_samples=5)
    /// >>> samples.num_samples()
    /// 5
    #[pyo3(signature = (
        rng,
        *,
        num_different_samples = *ommx::random::SamplesParameters::default().num_different_samples(),
        num_samples = *ommx::random::SamplesParameters::default().num_samples(),
        max_sample_id = None
    ))]
    pub fn random_samples(
        &self,
        rng: &Rng,
        num_different_samples: usize,
        num_samples: usize,
        max_sample_id: Option<u64>,
    ) -> OmmxPyResult<crate::Samples> {
        let max_sample_id = max_sample_id.unwrap_or(num_samples as u64);
        let params = ommx::random::SamplesParameters::new(
            num_different_samples,
            num_samples,
            max_sample_id,
        )?;

        let strategy = self.inner.arbitrary_samples(params);
        let mut rng_guard = rng.lock()?;
        let samples = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::Samples(samples))
    }

    #[doc = include_str!("docstrings/ommx/Instance.relax_constraint.md")]
    #[pyo3(signature = (constraint_id, reason, **parameters))]
    pub(crate) fn relax_constraint(
        &mut self,
        constraint_id: ConstraintIDInput,
        reason: String,
        #[gen_stub(override_type(type_repr = "str"))] parameters: Option<HashMap<String, String>>,
    ) -> OmmxPyResult<()> {
        self.inner.relax_constraint(
            constraint_id.into(),
            reason,
            parameters.unwrap_or_default(),
        )?;
        Ok(())
    }

    /// Make a retained removed regular constraint active again.
    ///
    /// Applies the Instance's current variable dependencies and fixed values
    /// before restoring the row, and removes its removal reason. The ID,
    /// labels, and provenance are retained. The argument accepts a
    /// {class}`~ommx.ConstraintID` or its integer value.
    ///
    /// This restores one condition, not the state before a transformation:
    /// generated variables or constraints and objective changes remain.
    /// See {meth}`~ommx.Instance.relax_constraint` for a complete round trip.
    /// Raises RuntimeError if the ID is not a removed regular constraint or
    /// its expression cannot be normalized for the current Instance. Failure
    /// leaves the row removed.
    pub(crate) fn restore_constraint(
        &mut self,
        constraint_id: ConstraintIDInput,
    ) -> OmmxPyResult<()> {
        self.inner.restore_constraint(constraint_id.into())?;
        Ok(())
    }

    /// Relax an indicator constraint by moving it from active to removed.
    ///
    /// Accepts an {class}`~ommx.IndicatorConstraintID` or its integer value.
    /// Records the supplied reason and string-valued keyword parameters,
    /// preserving the constraint's ID, labels, and provenance. Raises
    /// RuntimeError if the ID does not identify an active Indicator constraint.
    /// Evaluation still checks the retained condition; see
    /// {attr}`~ommx.Instance.removed_constraints` for feasibility semantics and
    /// {meth}`~ommx.Instance.relax_constraint` for the equivalent regular-
    /// constraint workflow. Restore with
    /// {meth}`~ommx.Instance.restore_indicator_constraint`.
    #[pyo3(signature = (constraint_id, reason, **parameters))]
    pub(crate) fn relax_indicator_constraint(
        &mut self,
        constraint_id: IndicatorConstraintIDInput,
        reason: String,
        #[gen_stub(override_type(type_repr = "str"))] parameters: Option<HashMap<String, String>>,
    ) -> OmmxPyResult<()> {
        self.inner.relax_indicator_constraint(
            constraint_id.into(),
            reason,
            parameters.unwrap_or_default(),
        )?;
        Ok(())
    }

    /// Restore a removed indicator constraint back to active.
    ///
    /// Accepts an {class}`~ommx.IndicatorConstraintID` or its integer value.
    /// Applies current fixed values and dependencies to the constraint body,
    /// preserving its ID, labels, and provenance and clearing the removal
    /// reason. This does not undo generated constraints, auxiliary variables,
    /// or objective changes from other transformations.
    ///
    /// Raises RuntimeError if the ID is not a removed Indicator constraint,
    /// its controlling variable has been fixed or substituted, or its body
    /// cannot be normalized. Failure leaves the row removed.
    pub(crate) fn restore_indicator_constraint(
        &mut self,
        constraint_id: IndicatorConstraintIDInput,
    ) -> OmmxPyResult<()> {
        self.inner
            .restore_indicator_constraint(constraint_id.into())?;
        Ok(())
    }

    /// Convert a one-hot constraint to a regular equality constraint.
    ///
    /// A one-hot constraint over ``{x_1, ..., x_n}`` is mathematically equivalent to the
    /// linear equality ``x_1 + ... + x_n - 1 == 0``. This method inserts that equality
    /// as a new regular constraint and moves the one-hot constraint into
    /// {attr}`~ommx.Instance.removed_constraints` with
    /// ``reason="ommx.Instance.convert_one_hot_to_constraint"`` and a
    /// ``constraint_id`` parameter pointing to the new regular constraint.
    /// The generated regular constraint preserves the original one-hot context
    /// and appends a {class}`~ommx.Provenance` entry identifying the original
    /// one-hot constraint.
    ///
    /// Returns the ID of the newly created regular constraint.
    ///
    /// Raises if the ID does not identify an active one-hot constraint, the
    /// equality cannot be constructed, or no fresh constraint ID is available.
    /// The instance is unchanged on error.
    ///
    /// # Examples
    ///
    /// >>> from ommx import Instance, DecisionVariable, OneHotConstraint, Sense
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     one_hot_constraints={1: OneHotConstraint(variables=x)},
    /// ...     sense=Sense.Minimize,
    /// ... )
    /// >>> new_id = instance.convert_one_hot_to_constraint(1)
    /// >>> instance.constraints
    /// {ConstraintID(2): Constraint(x0 + x1 + x2 - 1 == 0)}
    /// >>> instance.removed_constraints
    /// {OneHotConstraintID(1): RemovedOneHotConstraint(OneHotConstraint(exactly one of {x0, x1, x2} = 1), reason=ommx.Instance.convert_one_hot_to_constraint, constraint_id=2)}
    pub(crate) fn convert_one_hot_to_constraint(
        &mut self,
        one_hot_id: OneHotConstraintIDInput,
    ) -> OmmxPyResult<crate::ConstraintID> {
        let new_id = self
            .inner
            .convert_one_hot_to_constraint(one_hot_id.into())?;
        Ok(new_id.into())
    }

    /// Convert every active one-hot constraint to a regular equality constraint.
    ///
    /// See {meth}`~ommx.Instance.convert_one_hot_to_constraint` for the conversion rule.
    /// Returns the IDs of the newly created regular constraints in ascending
    /// order of the original one-hot IDs.
    ///
    /// Fresh constraint-ID capacity is checked for the entire family before
    /// mutation; insufficient capacity leaves the instance unchanged.
    /// Conversions are then applied one at a time in that order. Other failures
    /// do not provide family-wide rollback: earlier completed conversions remain.
    ///
    /// # Examples
    ///
    /// >>> from ommx import Instance, DecisionVariable, OneHotConstraint, Sense
    /// >>> x = [DecisionVariable.binary(i) for i in range(4)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     one_hot_constraints={
    /// ...         1: OneHotConstraint(variables=x[:2]),
    /// ...         2: OneHotConstraint(variables=x[2:]),
    /// ...     },
    /// ...     sense=Sense.Minimize,
    /// ... )
    /// >>> instance.convert_all_one_hots_to_constraints()
    /// [ConstraintID(3), ConstraintID(4)]
    /// >>> instance.constraints
    /// {ConstraintID(3): Constraint(x0 + x1 - 1 == 0), ConstraintID(4): Constraint(x2 + x3 - 1 == 0)}
    pub fn convert_all_one_hots_to_constraints(
        &mut self,
    ) -> OmmxPyResult<Vec<crate::ConstraintID>> {
        let ids = self.inner.convert_all_one_hots_to_constraints()?;
        Ok(ids.into_iter().map(|id| id.into()).collect())
    }

    /// Convert a SOS1 constraint to regular constraints using the Big-M method.
    ///
    /// A SOS1 constraint over $\{x_1, \ldots, x_n\}$ with each $x_i \in [l_i, u_i]$
    /// asserts that at most one $x_i$ is non-zero. Per variable, a binary indicator
    /// $y_i$ is introduced with the Big-M pair
    ///
    /// $$
    /// x_i - u_i y_i \leq 0, \qquad l_i y_i - x_i \leq 0
    /// $$
    ///
    /// (trivial sides $u_i = 0$ or $l_i = 0$ are skipped), together with the single
    /// cardinality constraint
    ///
    /// $$
    /// \sum_i y_i - 1 \leq 0.
    /// $$
    ///
    /// If $x_i$ is already binary with bound $[0, 1]$, $x_i$ itself is reused as its
    /// indicator (no new variable, no Big-M pair).
    ///
    /// Returns the list of newly created regular constraint IDs in insertion order
    /// (Big-M upper/lower pairs per non-binary variable, followed by the cardinality
    /// sum).
    ///
    /// Raises if any $x_i$ has a non-binary bound that is not finite, if its domain
    /// excludes $0$, or if its kind is semi-continuous / semi-integer (the split
    /// domain $\{0\} \cup [l, u]$ is not uniformly implemented across the codebase
    /// yet, so Big-M conversion of these kinds is not supported), or if there
    /// is insufficient decision-variable or regular-constraint ID capacity.
    /// All preconditions, including generated-ID capacity, are validated before
    /// mutation. Any error leaves the Instance unchanged.
    ///
    /// On success, the original SOS1 constraint is moved to
    /// {attr}`~ommx.Instance.removed_constraints` with
    /// ``reason="ommx.Instance.convert_sos1_to_constraints"`` and a
    /// comma-separated ``constraint_ids`` parameter in returned order. Every
    /// generated regular constraint records a {class}`~ommx.Provenance` entry
    /// identifying the original SOS1 constraint.
    ///
    /// # Examples
    ///
    /// All-binary SOS1 reduces to ``sum(x_i) - 1 <= 0`` without extra variables:
    ///
    /// >>> from ommx import Instance, DecisionVariable, Sense, Sos1Constraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     sos1_constraints={1: Sos1Constraint(variables=x)},
    /// ...     sense=Sense.Minimize,
    /// ... )
    /// >>> instance.convert_sos1_to_constraints(1)
    /// [ConstraintID(2)]
    /// >>> instance.constraints
    /// {ConstraintID(2): Constraint(x0 + x1 + x2 - 1 <= 0)}
    /// >>> instance.removed_constraints
    /// {Sos1ConstraintID(1): RemovedSos1Constraint(Sos1Constraint(at most one of {x0, x1, x2} ≠ 0), reason=ommx.Instance.convert_sos1_to_constraints, constraint_ids=2)}
    pub(crate) fn convert_sos1_to_constraints(
        &mut self,
        sos1_id: Sos1ConstraintIDInput,
    ) -> OmmxPyResult<Vec<crate::ConstraintID>> {
        let new_ids = self.inner.convert_sos1_to_constraints(sos1_id.into())?;
        Ok(new_ids.into_iter().map(|id| id.into()).collect())
    }

    /// Convert every active SOS1 constraint to regular constraints using Big-M.
    ///
    /// See {meth}`~ommx.Instance.convert_sos1_to_constraints` for the conversion
    /// rule. Returns a dict mapping each original SOS1 ID to the list of regular
    /// constraint IDs it produced.
    ///
    /// Atomic: every active SOS1 and the total generated-ID capacity are
    /// validated before mutation. If validation fails, including because IDs
    /// are exhausted, the Instance is left unchanged.
    ///
    /// # Examples
    ///
    /// >>> from ommx import Instance, DecisionVariable, Sense, Sos1Constraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(4)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={},
    /// ...     sos1_constraints={
    /// ...         1: Sos1Constraint(variables=x[:2]),
    /// ...         2: Sos1Constraint(variables=x[2:]),
    /// ...     },
    /// ...     sense=Sense.Minimize,
    /// ... )
    /// >>> instance.convert_all_sos1_to_constraints()
    /// {Sos1ConstraintID(1): [ConstraintID(3)], Sos1ConstraintID(2): [ConstraintID(4)]}
    /// >>> instance.constraints
    /// {ConstraintID(3): Constraint(x0 + x1 - 1 <= 0), ConstraintID(4): Constraint(x2 + x3 - 1 <= 0)}
    pub fn convert_all_sos1_to_constraints(
        &mut self,
    ) -> OmmxPyResult<BTreeMap<crate::Sos1ConstraintID, Vec<crate::ConstraintID>>> {
        let result = self.inner.convert_all_sos1_to_constraints()?;
        Ok(result
            .into_iter()
            .map(|(id, ids)| (id.into(), ids.into_iter().map(|c| c.into()).collect()))
            .collect())
    }

    /// Convert an indicator constraint to regular constraints using the Big-M method.
    ///
    /// An indicator constraint ``y = 1 → f(x) <= 0`` (or ``= 0``) is encoded with
    /// upper and lower Big-M sides computed from the interval bounds of $f(x)$:
    ///
    /// $$
    /// f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0,
    /// $$
    ///
    /// where $u \geq \sup f(x)$ and $l \leq \inf f(x)$ are the upper and lower
    /// bounds of $f$ over the decision variables' domains.
    ///
    /// Side emission:
    ///
    /// - For ``<=`` indicators, only the upper side is considered; it is emitted
    ///   iff $u > 0$. If $u \leq 0$ the constraint is already implied by the
    ///   variable bounds and no Big-M is emitted.
    /// - For ``=`` indicators, both sides are considered independently: upper
    ///   emitted iff $u > 0$, lower emitted iff $l < 0$.
    ///
    /// When an equality side is skipped, the remaining constraints still enforce
    /// the implication correctly because the skipped inequality is already implied
    /// by the variable bounds: e.g. $u \leq 0$ together with the emitted lower side
    /// forces $f(x) = 0$ at $y = 1$ when $u = 0$, or renders $y = 1$ infeasible
    /// when $u < 0$ (correctly reflecting that $f(x) = 0$ has no solution under the
    /// given bounds). When both $u = 0$ and $l = 0$, the bound says $f(x) \equiv 0$
    /// so the equality is vacuously satisfied and nothing is emitted.
    ///
    /// Returns the list of newly created regular constraint IDs in insertion order
    /// (upper first, then lower). The list is empty when both sides are redundant.
    ///
    /// Raises if the bound needed for an emitted side is non-finite, or if $f(x)$
    /// references a semi-continuous / semi-integer variable (the split domain
    /// $\{0\} \cup [l, u]$ is not uniformly implemented, so Big-M conversion could
    /// silently drop the upper side when $0 \notin [l, u]$). Also raises if the
    /// ID is not active or there are insufficient fresh constraint IDs for the
    /// emitted sides. A conversion emitting no sides needs no fresh IDs.
    /// The instance is not mutated on error.
    ///
    /// ``atol`` controls which Function-body values the bound evaluator treats
    /// as zero. If omitted, :attr:`DEFAULT_ATOL` is used. Big-M algebra assumes
    /// the indicator variable is exactly binary; this does not canonicalize an
    /// approximate solver value near 0 or 1.
    ///
    /// On success, the original indicator constraint is moved to
    /// {attr}`~ommx.Instance.removed_constraints` with
    /// ``reason="ommx.Instance.convert_indicator_to_constraint"`` and a
    /// comma-separated ``constraint_ids`` parameter in returned order (empty
    /// when no side is emitted). Every generated regular constraint records a
    /// {class}`~ommx.Provenance` entry identifying the original indicator
    /// constraint.
    ///
    /// # Examples
    ///
    /// Convert an inequality indicator where the upper side is active:
    ///
    /// >>> from ommx import (
    /// ...     Instance, DecisionVariable, IndicatorConstraint, Equality, Sense,
    /// ... )
    /// >>> x = DecisionVariable.continuous(0, lower=0.0, upper=5.0)
    /// >>> y = DecisionVariable.binary(1)
    /// >>> ic = IndicatorConstraint(
    /// ...     indicator_variable=y,
    /// ...     function=x - 2,
    /// ...     equality=Equality.LessThanOrEqualToZero,
    /// ... )
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x, y],
    /// ...     objective=x,
    /// ...     constraints={},
    /// ...     indicator_constraints={1: ic},
    /// ...     sense=Sense.Minimize,
    /// ... )
    /// >>> instance.convert_indicator_to_constraint(1)
    /// [ConstraintID(2)]
    /// >>> instance.constraints
    /// {ConstraintID(2): Constraint(x0 + 3*x1 - 5 <= 0)}
    #[pyo3(signature = (indicator_id, *, atol=None))]
    pub(crate) fn convert_indicator_to_constraint(
        &mut self,
        indicator_id: IndicatorConstraintIDInput,
        atol: Option<f64>,
    ) -> OmmxPyResult<Vec<crate::ConstraintID>> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let new_ids = self
            .inner
            .convert_indicator_to_constraint(indicator_id.into(), atol)?;
        Ok(new_ids.into_iter().map(|id| id.into()).collect())
    }

    /// Convert every active indicator constraint to regular constraints using Big-M.
    ///
    /// See {meth}`~ommx.Instance.convert_indicator_to_constraint` for the
    /// conversion rule. Returns a dict mapping each original indicator ID to the
    /// list of regular constraint IDs it produced.
    ///
    /// Atomic: every active indicator is validated up front, and only if every
    /// one is convertible are the conversions applied. If any indicator fails
    /// validation, or there are insufficient fresh constraint IDs for all
    /// emitted sides, no mutation happens and the instance is left untouched.
    ///
    /// ``atol`` has the same Function-body meaning as in the single-constraint
    /// conversion. If omitted, :attr:`DEFAULT_ATOL` is used.
    #[pyo3(signature = (*, atol=None))]
    pub fn convert_all_indicators_to_constraints(
        &mut self,
        atol: Option<f64>,
    ) -> OmmxPyResult<BTreeMap<crate::IndicatorConstraintID, Vec<crate::ConstraintID>>> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let result = self.inner.convert_all_indicators_to_constraints(atol)?;
        Ok(result
            .into_iter()
            .map(|(id, ids)| (id.into(), ids.into_iter().map(|c| c.into()).collect()))
            .collect())
    }

    /// Log-encode the integer decision variables.
    ///
    /// Log encoding of an integer variable $x \in [l, u]$ is to represent by $m$ bits $b_i \in \{0, 1\}$ by:
    ///
    /// $$x = \sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l$$
    ///
    /// where $m = \lceil \log_2(u - l + 1) \rceil$.
    ///
    /// **Args:**
    /// - `decision_variable_ids`: The IDs of the integer decision variables to log-encode.
    ///   If not specified (or empty), all used integer variables are log-encoded.
    /// - `atol`: Optional absolute tolerance used when normalizing integer
    ///   bounds before encoding. If None, uses the default tolerance.
    ///
    /// Raises {class}`~ommx.LogEncodingError` when an exact representation is
    /// unavailable for a requested variable. Allocation and expression-rewrite
    /// failures retain their original exception types.
    ///
    /// # Postconditions
    ///
    /// Encoding preserves existing output semantics, or captures the pre-encoding active objective when no output objective exists, while rewriting the active objective.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.integer(0, lower=0, upper=3)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> instance.log_encode({0})
    /// >>> encoded_ids = instance.required_ids()
    /// >>> assert len(encoded_ids) == 2
    /// >>> state = {variable_id: 1 for variable_id in encoded_ids}
    /// >>> assert instance.objective.evaluate(state) == 3.0
    /// >>> solution = instance.evaluate(state)
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)
    #[pyo3(signature = (decision_variable_ids=BTreeSet::new(), *, atol=None))]
    pub fn log_encode(
        &mut self,
        py: Python<'_>,
        decision_variable_ids: BTreeSet<u64>,
        atol: Option<f64>,
    ) -> OmmxPyResult<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let ids: BTreeSet<u64> = if decision_variable_ids.is_empty() {
            // Auto-detect: find all used integer decision variables
            let usage = self.inner.decision_variable_usage();
            let integer_ids: BTreeSet<u64> = usage
                .used_integer()
                .into_keys()
                .map(|id| id.into_inner())
                .collect();
            if integer_ids.is_empty() {
                return Ok(());
            }
            integer_ids
        } else {
            decision_variable_ids
        };
        self.inner
            .log_encode(ids.iter().map(|id| (*id).into()), atol)?;
        Ok(())
    }

    /// Unary-encode the integer decision variables.
    ///
    /// Unary encoding of an integer variable $x \in [l, u]$ is to represent it
    /// by $u - l$ bits $b_j \in \{0, 1\}$:
    ///
    /// $$x = l + \sum_j b_j$$
    ///
    /// Every bit configuration maps to a valid integer in the original range,
    /// so no encoding-validity penalty or linking constraint is added. This
    /// costs linearly many auxiliary variables, so use it for narrow integer
    /// ranges.
    ///
    /// **Args:**
    /// - `decision_variable_ids`: The IDs of the integer decision variables to unary-encode.
    ///   If not specified (or empty), all used integer variables are unary-encoded.
    /// - `max_range`: Maximum allowed `upper - lower` range for each encoded
    ///   variable. This also bounds the number of auxiliary binary variables
    ///   introduced per integer variable.
    /// - `atol`: Optional absolute tolerance used when normalizing integer
    ///   bounds before encoding. If None, uses the default tolerance.
    ///
    /// # Postconditions
    ///
    /// Encoding preserves existing output semantics, or captures the pre-encoding active objective when no output objective exists, while rewriting the active objective.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.integer(0, lower=2, upper=5, name="x")
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> instance.unary_encode({0})
    /// >>> encoded_ids = instance.required_ids()
    /// >>> assert len(encoded_ids) == 3
    /// >>> state = {variable_id: 1 for variable_id in encoded_ids}
    /// >>> assert instance.objective.evaluate(state) == 5.0
    /// >>> solution = instance.evaluate(state)
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 5.0)
    #[pyo3(signature = (decision_variable_ids=BTreeSet::new(), *, max_range=16, atol=None))]
    pub fn unary_encode(
        &mut self,
        py: Python<'_>,
        decision_variable_ids: BTreeSet<u64>,
        max_range: usize,
        atol: Option<f64>,
    ) -> OmmxPyResult<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let ids: BTreeSet<u64> = if decision_variable_ids.is_empty() {
            let usage = self.inner.decision_variable_usage();
            let integer_ids: BTreeSet<u64> = usage
                .used_integer()
                .into_keys()
                .map(|id| id.into_inner())
                .collect();
            if integer_ids.is_empty() {
                return Ok(());
            }
            integer_ids
        } else {
            decision_variable_ids
        };
        self.inner
            .unary_encode(ids.iter().map(|id| (*id).into()), max_range, atol)?;
        Ok(())
    }

    /// Substitute decision variables with function expressions (in-place).
    ///
    /// Replaces each given decision variable with the provided function in the
    /// objective and all active constraints. This is the general substitution
    /// mechanism behind {meth}`~ommx.Instance.log_encode`, exposed so that
    /// users can implement their own integer encodings (e.g. unary, one-hot).
    ///
    /// **Args:**
    /// - `assignments`: A dict mapping decision variable IDs to the function
    ///   expressions that should replace them.
    ///
    /// **Important:**
    /// This method performs an algebraic rewrite. It does not automatically
    /// translate the substituted variable's bound or kind into constraints on
    /// the replacement expression. For example, substituting a binary variable
    /// ``x`` with ``y + z`` does not add ``0 <= y + z <= 1``, and substituting
    /// an integer variable does not ensure that the replacement expression is
    /// integral. If the substitution must preserve the optimization problem,
    /// the caller must provide a domain-preserving encoding or add the required
    /// linking and bound constraints explicitly.
    ///
    /// The substituted variable's definition is retained and its value becomes
    /// dependent on the replacement expression. {meth}`~ommx.Instance.evaluate`
    /// reconstructs that value and checks its original kind and bound when
    /// determining {attr}`~ommx.Solution.feasible`. Those checks do not add
    /// constraints to the solver input. Being able to reconstruct a value is
    /// therefore not a guarantee of an equivalent optimization problem.
    ///
    /// Raises ``ValueError`` on cyclic or recursive assignments, or when
    /// substituting the controlling variable of an active Indicator constraint
    /// or a member of an active OneHot or SOS1 constraint. Variables appearing
    /// only in an Indicator's function body can be substituted.
    ///
    /// # Examples
    ///
    /// To retain a variable's domain in the solver input, keep that variable
    /// and add a linking equality. Here two newly introduced binary variables
    /// encode one original binary variable. The original variable's domain
    /// and the equality together prevent both encoding variables from being 1.
    ///
    /// >>> from ommx import Instance
    /// >>> linked = Instance.maximize()
    /// >>> original = linked.new_binary("original")
    /// >>> linked.objective = original
    /// >>> bits = [linked.new_binary("bit", subscripts=[i]) for i in range(2)]
    /// >>> _ = linked.add_constraint(original == bits[0] + bits[1])
    /// >>> assert linked.evaluate({original.id: 1, bits[0].id: 1, bits[1].id: 0}).feasible
    /// >>> assert not linked.evaluate({original.id: 1, bits[0].id: 1, bits[1].id: 1}).feasible
    ///
    /// Alternatively, eliminate the original variable from the solver input
    /// and constrain the replacement expression explicitly. Because the new
    /// variables are binary, their sum is integral; the added bounds ensure
    /// that it also lies in the original binary domain. The lower bound is
    /// redundant for this particular encoding, but illustrates both sides of
    /// the domain constraint.
    ///
    /// >>> from ommx import Instance
    /// >>> encoded = Instance.maximize()
    /// >>> original = encoded.new_binary("original")
    /// >>> encoded.objective = original
    /// >>> bits = [encoded.new_binary("bit", subscripts=[i]) for i in range(2)]
    /// >>> expression = bits[0] + bits[1]
    /// >>> encoded.substitute({original.id: expression})
    /// >>> _ = encoded.add_constraint(expression >= 0)
    /// >>> _ = encoded.add_constraint(expression <= 1)
    /// >>> solution = encoded.evaluate({bits[0].id: 1, bits[1].id: 0})
    /// >>> assert solution.feasible
    /// >>> assert solution.state.entries[original.id] == 1.0
    /// >>> assert not encoded.evaluate({bits[0].id: 1, bits[1].id: 1}).feasible
    ///
    /// # Postconditions
    ///
    /// Substitution rewrites the active objective while output evaluation restores the substituted variable value.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> b = DecisionVariable.binary(1)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x, b], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> instance.substitute({0: b})
    /// >>> assert instance.required_ids() == {1}
    /// >>> assert instance.objective.evaluate({1: 1}) == -1.0
    /// >>> solution = instance.evaluate({1: 1})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 1.0)
    #[pyo3(signature = (assignments))]
    pub fn substitute(
        &mut self,
        py: Python<'_>,
        assignments: HashMap<u64, Function>,
    ) -> OmmxPyResult<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let iter = assignments
            .into_iter()
            .map(|(id, f)| (VariableID::from(id), f.0));
        ommx::substitute(&mut self.inner, iter)?;
        Ok(())
    }

    /// Convert an inequality constraint $f(x) \leq 0$ to an equality constraint $f(x) + s/a = 0$ with an integer slack variable $s$.
    ///
    /// - Since $a$ is determined as the minimal multiplier to make every coefficient of $a f(x)$ integer,
    ///   $a$ itself and the range of $s$ becomes impractically large. ``max_integer_range`` limits the maximal
    ///   range of $s$, and returns error if the range exceeds it.
    ///
    /// - Since this method evaluates the bound of $f(x)$, we may find that:
    ///
    ///   - The bound $[l, u]$ is infeasible at the selected tolerance, i.e.
    ///     $l > \text{atol}$:
    ///     this means the instance is infeasible because this constraint never be satisfied,
    ///     and an error is raised.
    ///
    ///   - The bound is feasible everywhere at the selected tolerance, i.e.
    ///     $u \leq \text{atol}$:
    ///     this means this constraint is trivially satisfied,
    ///     the constraint is moved to {attr}`~ommx.Instance.removed_constraints`,
    ///     and this method returns without introducing slack variable or raising an error.
    ///
    /// # Examples
    ///
    /// Let's consider a simple inequality constraint x0 + 2*x1 <= 5.
    ///
    /// >>> from ommx import ConstraintID
    /// >>> from ommx import DecisionVariable, Equality, Instance, Sense
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={0: x[0] + 2*x[1] <= 5},
    /// ...     sense=Sense.Maximize,
    /// ... )
    ///
    /// Introduce an integer slack variable
    ///
    /// >>> instance.convert_inequality_to_equality_with_integer_slack(
    /// ...     constraint_id=0,
    /// ...     max_integer_range=32
    /// ... )
    /// >>> assert instance.constraints[ConstraintID(0)].function.terms == {
    /// ...     (0,): 1.0, (1,): 2.0, (3,): 1.0, (): -5.0
    /// ... }
    /// >>> assert instance.constraints[ConstraintID(0)].equality == Equality.EqualToZero
    ///
    /// Raises {class}`~ommx.ExactIntegerSlackError` when exact conversion is
    /// unavailable because the coefficients cannot be normalized or the slack
    /// range exceeds ``max_integer_range``. Raises
    /// {class}`~ommx.InfeasibleDetected` when the bounds prove the inequality
    /// infeasible.
    /// ``atol`` controls zero-sensitive interval evaluation and the inclusive
    /// inequality feasibility threshold and must be less than ``0.5``. If
    /// omitted, :attr:`DEFAULT_ATOL` is used.
    #[pyo3(signature = (constraint_id, max_integer_range, *, atol=None))]
    pub(crate) fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: ConstraintIDInput,
        max_integer_range: u64,
        atol: Option<f64>,
    ) -> OmmxPyResult<()> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        self.inner
            .convert_inequality_to_equality_with_integer_slack(
                ommx::ConstraintID::from(constraint_id).into_inner(),
                max_integer_range,
                atol,
            )?;
        Ok(())
    }

    /// Convert inequality $f(x) \leq 0$ to **inequality** $f(x) + b s \leq 0$ with an integer slack variable $s$.
    ///
    /// - This should be used when {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack` is not applicable.
    ///
    /// - The bound of $s$ will be $[0, \text{slack\_upper\_bound}]$, and the coefficient $b$ is determined from the lower bound of $f(x)$.
    ///
    /// - Since the slack variable is integer, the yielded inequality has residual error $\min_s f(x) + b s$ at most $b$.
    ///   And thus $b$ is returned to use scaling the penalty weight or other things.
    ///
    ///   - Larger slack_upper_bound (i.e. finer-grained slack) yields smaller $b$, and thus smaller the residual error,
    ///     but it needs more bits for the slack variable, and thus the problem size becomes larger.
    ///
    /// **Returns:**
    /// The coefficient $b$ of the slack variable. If the constraint is trivially satisfied, this returns ``None``.
    ///
    /// ``atol`` controls zero-sensitive interval bounds and the inclusive
    /// inequality feasibility threshold used to select the slack coefficient;
    /// it must be less than ``0.5``. If omitted, :attr:`DEFAULT_ATOL` is used.
    ///
    /// # Examples
    ///
    /// Let's consider a simple inequality constraint x0 + 2*x1 <= 4.
    ///
    /// >>> from ommx import ConstraintID
    /// >>> from ommx import DecisionVariable, Equality, Instance, Sense
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints={0: x[0] + 2*x[1] <= 4},
    /// ...     sense=Sense.Maximize,
    /// ... )
    ///
    /// Introduce an integer slack variable s in [0, 2]
    ///
    /// >>> b = instance.add_integer_slack_to_inequality(
    /// ...     constraint_id=0,
    /// ...     slack_upper_bound=2
    /// ... )
    /// >>> assert b == 2.0
    /// >>> assert instance.constraints[ConstraintID(0)].function.terms == {
    /// ...     (0,): 1.0, (1,): 2.0, (3,): 2.0, (): -4.0
    /// ... }
    /// >>> assert instance.constraints[ConstraintID(0)].equality == Equality.LessThanOrEqualToZero
    #[pyo3(signature = (constraint_id, slack_upper_bound, *, atol=None))]
    pub(crate) fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: ConstraintIDInput,
        slack_upper_bound: u64,
        atol: Option<f64>,
    ) -> OmmxPyResult<Option<f64>> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        let result = self.inner.add_integer_slack_to_inequality(
            ommx::ConstraintID::from(constraint_id).into_inner(),
            slack_upper_bound,
            atol,
        )?;
        Ok(result)
    }

    /// Return the state role of a decision variable.
    ///
    /// See {class}`~ommx.DecisionVariableRole` for the four roles, which depend
    /// on this Instance's current structure rather than the variable's kind.
    /// Unknown IDs return ``None``. For a complete reconstruction example,
    /// see {meth}`~ommx.Instance.populate_state`.
    pub fn decision_variable_role(&self, id: u64) -> Option<DecisionVariableRole> {
        self.inner
            .decision_variable_role(VariableID::from(id))
            .map(Into::into)
    }

    /// Return the state role of every decision variable, keyed by ID.
    ///
    /// Each value is a {class}`~ommx.DecisionVariableRole` derived from this
    /// Instance. See {meth}`~ommx.Instance.populate_state` for an example that
    /// queries roles after substitution and reconstructs the candidate values.
    pub fn decision_variable_roles(&self) -> BTreeMap<u64, DecisionVariableRole> {
        self.inner
            .decision_variable_roles()
            .into_iter()
            .map(|(id, role)| (id.into_inner(), role.into()))
            .collect()
    }

    /// Return fixed decision variables as ``{id: fixed_value}``.
    ///
    /// These values are retained by the Instance and restored by
    /// {meth}`~ommx.Instance.populate_state`; a caller's fixed-value entry is
    /// checked for consistency rather than used to overwrite the recorded value.
    pub fn fixed_decision_variables(&self) -> BTreeMap<u64, f64> {
        self.inner
            .fixed_decision_variables()
            .into_iter()
            .map(|(id, value)| (id.into_inner(), value))
            .collect()
    }

    /// Return IDs of decision variables defined by ``decision_variable_dependency``.
    ///
    /// {meth}`~ommx.Instance.populate_state` computes their values from the
    /// recorded expressions. See that method for an example and consistency rules.
    pub fn dependent_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.inner
            .dependent_decision_variable_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Return IDs of decision variables not used, fixed, or dependent.
    ///
    /// Values omitted from a candidate receive defaults under
    /// {meth}`~ommx.Instance.populate_state`. Supply a value explicitly when
    /// it matters for reporting or checking a particular candidate.
    pub fn irrelevant_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.inner
            .irrelevant_decision_variable_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get statistics about the instance.
    ///
    /// Returns a dictionary containing counts of decision variables and constraints
    /// categorized by kind, usage, and status.
    ///
    /// **Returns:**
    /// A dictionary with the following structure:
    ///
    /// ```text
    /// {
    ///     "decision_variables": {
    ///         "total": int,
    ///         "by_kind": {
    ///             "binary": int,
    ///             "integer": int,
    ///             "continuous": int,
    ///             "semi_integer": int,
    ///             "semi_continuous": int
    ///         },
    ///         "by_usage": {
    ///             "used_in_objective": int,
    ///             "used_in_constraints": int,
    ///             "used": int,
    ///             "fixed": int,
    ///             "dependent": int,
    ///             "irrelevant": int
    ///         }
    ///     },
    ///     "constraints": {
    ///         "total": int,
    ///         "active": int,
    ///         "removed": int
    ///     }
    /// }
    /// ```
    ///
    /// # Examples
    ///
    /// >>> from ommx import Instance
    /// >>> instance = Instance.minimize()
    /// >>> stats = instance.stats()
    /// >>> stats["decision_variables"]["total"]
    /// 0
    /// >>> stats["constraints"]["total"]
    /// 0
    pub fn stats<'py>(&self, py: Python<'py>) -> OmmxPyResult<Bound<'py, PyDict>> {
        let stats = self.inner.stats();
        Ok(serde_pyobject::to_pyobject(py, &stats)?.extract()?)
    }

    /// DataFrame of decision variables
    #[pyo3(signature = (include = None))]
    pub fn decision_variables_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let label_store = self.inner.variable_labels();
        let roles = self.inner.decision_variable_roles();
        let entries = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| {
                let label = label_store.collect_for(*id);
                let dict = crate::pandas::WithModelingContext::new(
                    (*id, dv, self.inner.fixed_decision_variable_value(*id)),
                    &label,
                )
                .to_pandas_entry(py)?;
                let role = roles
                    .get(id)
                    .expect("role query uses the same decision_variables map");
                dict.set_item("state_role", role.as_str())?;
                apply_include_filter(&dict, flags)?;
                Ok(dict.into_any())
            })
            .collect::<PyResult<_>>()?;
        raw_entries_to_dataframe(py, entries, "id")
    }

    /// DataFrame of constraints, dispatched on `kind=`.
    ///
    /// `kind` selects the constraint family — `"regular"`, `"indicator"`,
    /// `"one_hot"`, or `"sos1"`. The DataFrame is indexed by the kind-
    /// qualified id column (`{kind}_constraint_id`).
    ///
    /// `include` selects which optional column families to fold in. It
    /// accepts a sequence of `"label"` / `"parameters"` /
    /// `"removed_reason"`; passing `None` (the default) yields the
    /// v2-equivalent shape (`label` + `parameters`). `"removed_reason"`
    /// is a unit flag that gates both the `removed_reason` column and the
    /// `removed_reason.{key}` parameter columns together.
    ///
    /// `removed=False` (default) returns active constraints only.
    /// `removed=True` returns active + removed rows in the same DataFrame
    /// and auto-sets `"removed_reason"` so removed rows are
    /// distinguishable (active rows have NA in the reason columns).
    #[pyo3(signature = (*, kind, include = None, removed = false))]
    pub fn constraints_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
        include: Option<Vec<String>>,
        removed: bool,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        let mut flags = crate::pandas::IncludeFlags::from_optional(include)?;
        if removed {
            flags.removed_reason = true;
        }
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                let meta = coll.context().clone();
                let active = coll.active();
                let removed_map = coll.removed();
                let mut entries: Vec<Bound<'py, pyo3::types::PyAny>> = Vec::new();
                if removed {
                    // Globally id-sorted merge of active and removed.
                    // Both BTreeMaps are individually sorted; ids are
                    // disjoint between the two maps.
                    let mut ai = active.iter().peekable();
                    let mut ri = removed_map.iter().peekable();
                    loop {
                        let pick_active = match (ai.peek(), ri.peek()) {
                            (Some((aid, _)), Some((rid, _))) => aid <= rid,
                            (Some(_), None) => true,
                            (None, Some(_)) => false,
                            (None, None) => break,
                        };
                        if pick_active {
                            let (id, c) = ai.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithModelingContext::new((*id, c), &m)
                                .to_pandas_entry(py)?;
                            // Active rows in a removed=True view get NA
                            // in the (always-present) removed_reason column.
                            crate::pandas::set_removed_reason_na(&dict)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        } else {
                            let (id, pair) = ri.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithModelingContext::new((*id, pair), &m)
                                .to_pandas_entry(py)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        }
                    }
                } else {
                    for (id, c) in active.iter() {
                        let m = meta.collect_for(*id);
                        let dict = crate::pandas::WithModelingContext::new((*id, c), &m)
                            .to_pandas_entry(py)?;
                        // User-requested removed_reason on an active-only
                        // view: ensure the column survives even when no
                        // row carries a reason.
                        if flags.removed_reason {
                            crate::pandas::set_removed_reason_na(&dict)?;
                        }
                        crate::pandas::apply_include_filter(&dict, flags)?;
                        crate::pandas::rename_id_column(&dict, id_col)?;
                        entries.push(dict.into_any());
                    }
                }
                crate::pandas::raw_entries_to_dataframe(py, entries, id_col)
            }
        )
    }

    /// DataFrame of named functions
    #[pyo3(signature = (include = None))]
    pub fn named_functions_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let nf_meta_store = self.inner.named_function_labels().clone();
        let nf_meta_view: Vec<(
            ommx::NamedFunctionLabel,
            ommx::NamedFunctionID,
            &ommx::NamedFunction,
        )> = self
            .inner
            .named_functions()
            .iter()
            .map(|(id, nf)| (nf_meta_store.collect_for(*id), *id, nf))
            .collect();
        entries_to_dataframe(
            py,
            nf_meta_view
                .iter()
                .map(|(m, id, nf)| crate::pandas::WithModelingContext::new((*id, *nf), m)),
            "id",
            flags,
        )
    }

    /// Constraint context DataFrame (id-indexed wide format).
    ///
    /// One row per constraint id (active + removed) with columns
    /// `name`, `subscripts`, `description`. Index column is
    /// `{kind}_constraint_id`. `kind` selects which constraint family
    /// to read: `"regular"`, `"indicator"`, `"one_hot"`, or `"sos1"`.
    #[pyo3(signature = (*, kind))]
    pub fn constraint_context_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_context_dataframe(
                    py,
                    coll.context(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint parameters DataFrame (long format).
    ///
    /// One row per (constraint_id, parameter_key) pair. Columns:
    /// `{kind}_constraint_id`, `key`, `value`. Default RangeIndex.
    #[pyo3(signature = (*, kind))]
    pub fn constraint_parameters_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_parameters_dataframe(
                    py,
                    coll.context(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint provenance DataFrame (long format).
    ///
    /// One row per (constraint_id, step) pair. Columns:
    /// `{kind}_constraint_id`, `step`, `source_kind`, `source_id`.
    #[pyo3(signature = (*, kind))]
    pub fn constraint_provenance_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_provenance_dataframe(
                    py,
                    coll.context(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Removed-constraint reasons DataFrame (long format).
    ///
    /// One row per (constraint_id, parameter_key) pair, plus one row with
    /// `key`/`value` set to NA when the reason has no parameters. Columns:
    /// `{kind}_constraint_id`, `reason`, `key`, `value`.
    #[pyo3(signature = (*, kind))]
    pub fn constraint_removed_reasons_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_removed_reasons_dataframe(
                    py,
                    coll.removed().iter().map(|(id, (_, r))| (*id, r)),
                    id_col,
                )
            }
        )
    }

    /// Decision-variable modeling-label DataFrame (id-indexed wide format).
    ///
    /// Columns: `name`, `subscripts`, `description`. Index column =
    /// `variable_id`.
    pub fn variable_labels_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_labels_dataframe(
            py,
            self.inner.variable_labels(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }

    /// Decision-variable parameters DataFrame (long format).
    ///
    /// One row per (variable_id, parameter_key) pair. Columns:
    /// `variable_id`, `key`, `value`.
    pub fn variable_parameters_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_parameters_dataframe(
            py,
            self.inner.variable_labels(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Convert the instance to a minimization problem.
    ///
    /// If both the active objective and the output objective already use
    /// minimization, this does nothing.
    ///
    /// **Returns:**
    /// ``True`` if either objective is converted, ``False`` if both already
    /// use minimization.
    ///
    /// # Postconditions
    ///
    /// Conversion changes both active and output objective semantics and is idempotent at the target sense. An existing output objective remains explicit even if both objectives become structurally equal.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Minimize)
    /// >>> assert instance.evaluate({0: 1}).objective == 3.0
    /// >>> assert instance.as_minimization_problem()
    /// >>> solution = instance.evaluate({0: 1})
    /// >>> assert instance.objective.evaluate({0: 1}) == -3.0
    /// >>> assert (solution.sense, solution.objective) == (Sense.Minimize, -3.0)
    /// >>> assert not instance.as_minimization_problem()
    pub fn as_minimization_problem(&mut self) -> bool {
        self.inner.as_minimization_problem()
    }

    /// Convert the instance to a maximization problem.
    ///
    /// If both the active objective and the output objective already use
    /// maximization, this does nothing.
    ///
    /// **Returns:**
    /// ``True`` if either objective is converted, ``False`` if both already
    /// use maximization.
    ///
    /// # Postconditions
    ///
    /// Conversion changes both active and output objective semantics and is idempotent at the target sense. An existing output objective remains explicit even if both objectives become structurally equal.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Minimize
    /// ... )
    /// >>> assert instance.convert_active_objective(Sense.Maximize)
    /// >>> assert instance.evaluate({0: 1}).objective == 3.0
    /// >>> assert instance.as_maximization_problem()
    /// >>> solution = instance.evaluate({0: 1})
    /// >>> assert instance.objective.evaluate({0: 1}) == -3.0
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, -3.0)
    /// >>> assert not instance.as_maximization_problem()
    pub fn as_maximization_problem(&mut self) -> bool {
        self.inner.as_maximization_problem()
    }

    #[doc = include_str!("docstrings/ommx/Instance.convert_active_objective.md")]
    pub fn convert_active_objective(&mut self, target: Sense) -> bool {
        self.inner.convert_active_objective(target.into())
    }

    /// Map an optimality status for the active solver-facing formulation to
    /// the objective semantics returned by evaluation.
    ///
    /// When the instance records that active-formulation optimality does not
    /// transport to its output objective, this returns
    /// {attr}`~ommx.Optimality.Unspecified`.
    /// Otherwise the supplied status is returned unchanged. This reads the
    /// recorded transformation guarantee; it does not solve the problem,
    /// verify a backend's claim, or test a candidate's feasibility. Merely
    /// retaining an output objective or reconstructing variable values does
    /// not establish optimality. See {attr}`~ommx.Solution.optimality` for the
    /// status stored on a result.
    ///
    /// # Postconditions
    ///
    /// Optimality is preserved for equivalent objective conversion and discarded after penalty preparation.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Optimality, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> equivalent = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert equivalent.convert_active_objective(Sense.Minimize)
    /// >>> statuses = (Optimality.Unspecified, Optimality.Optimal, Optimality.NotOptimal)
    /// >>> for status in statuses:
    /// ...     assert equivalent.map_active_optimality(status) == status
    /// >>> penalized = Instance.from_components(
    /// ...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Minimize
    /// ... )
    /// >>> _ = penalized.to_qubo(uniform_penalty_weight=1.0)
    /// >>> for status in statuses:
    /// ...     assert penalized.map_active_optimality(status) == Optimality.Unspecified
    pub fn map_active_optimality(&self, active: crate::Optimality) -> crate::Optimality {
        self.inner.map_active_optimality(active.into()).into()
    }

    /// Get a specific decision variable by ID
    pub fn get_decision_variable_by_id(&self, variable_id: u64) -> PyResult<DecisionVariable> {
        let var_id = VariableID::from(variable_id);
        let labels = self.inner.variable_labels();
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|var| {
                DecisionVariable::from_parts(var_id, var.clone(), labels.collect_for(var_id))
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific named function by ID
    pub fn get_named_function_by_id(&self, named_function_id: u64) -> PyResult<NamedFunction> {
        let id = NamedFunctionID::from(named_function_id);
        self.inner
            .named_functions()
            .get(&id)
            .map(|named_function| {
                NamedFunction(
                    id,
                    named_function.clone(),
                    self.inner.named_function_labels().collect_for(id),
                )
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Named function with ID {named_function_id} not found"
                ))
            })
    }

    /// Reduce binary powers in the instance.
    ///
    /// This method replaces binary powers in the instance with their equivalent linear expressions.
    /// For binary variables, $x^n = x$ for any $n \geq 1$, so we can reduce higher powers to linear terms.
    ///
    /// **Returns:**
    /// ``True`` if any reduction was performed, ``False`` otherwise.
    ///
    /// # Postconditions
    ///
    /// Reduction preserves existing output semantics, or captures the pre-reduction active objective when no output objective exists, while rewriting active expressions.
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = DecisionVariable.binary(0)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x], objective=x * x * x, constraints={}, sense=Sense.Maximize
    /// ... )
    /// >>> assert instance.reduce_binary_power()
    /// >>> assert instance.objective.evaluate({0: 1}) == 1.0
    /// >>> solution = instance.evaluate({0: 1})
    /// >>> assert (solution.sense, solution.objective) == (Sense.Maximize, 1.0)
    /// >>> assert not instance.reduce_binary_power()
    pub fn reduce_binary_power(&mut self) -> OmmxPyResult<bool> {
        Ok(self.inner.reduce_binary_power()?)
    }

    #[staticmethod]
    pub fn load_mps(py: Python<'_>, path: String) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let instance = ommx::mps::load(path)?;
        Ok(Self { inner: instance })
    }

    /// Write the active formulation to an MPS file.
    ///
    /// # Parameters
    ///
    /// - ``path``: Destination file. Missing parent directories are created and
    ///   an existing file is overwritten after model validation succeeds.
    /// - ``compress``: Write gzip-compressed bytes when ``True`` (the default),
    ///   or plain text when ``False``, independently of the filename extension.
    ///
    /// # Supported models
    ///
    /// The writer accepts both minimization and maximization. The active objective
    /// and active regular constraint functions must be stored as expanded
    /// polynomials of degree at most two. Quadratic terms are written using
    /// ``QUADOBJ`` and ``QCMATRIX`` sections, with the coefficient convention
    /// described below. Variables used by the active formulation must have
    /// Binary, Integer, or Continuous kind. Active Indicator, OneHot, and SOS1
    /// constraints are rejected; lower them before export if appropriate.
    /// No preparation is performed automatically.
    ///
    /// Unsupported models raise ``RuntimeError`` before creating directories
    /// or truncating the destination. Errors reported during path setup or
    /// writing raise ``RuntimeError``. Writing is not atomic: created directories
    /// and a truncated or partial destination may remain after failure.
    /// With compression enabled, the current implementation does not propagate
    /// errors that occur only while finishing the gzip stream. A normal return
    /// therefore does not guarantee a complete gzip file. The instance is not
    /// modified.
    ///
    /// # Quadratic coefficient convention
    ///
    /// The current writer emits each stored quadratic monomial coefficient once,
    /// unchanged, in both sections. It does not double diagonal objective
    /// coefficients or add mirrored off-diagonal constraint entries.
    /// Receiving tools can therefore interpret a different model or reject the
    /// file even when they recognize these section names.
    ///
    /// For example, [Gurobi's MPS convention](https://docs.gurobi.com/projects/optimizer/en/current/reference/fileformats/modelformats.html#quadobj-section)
    /// applies a one-half multiplier to the objective matrix and requires a
    /// symmetric constraint matrix. An objective ``6 * x * x`` is written here
    /// as a diagonal coefficient of ``6``, which that objective convention
    /// interprets as ``3 * x * x``. Check the receiving tool's conventions and
    /// imported quadratic expressions before solving; this writer does not
    /// adapt the coefficients to a selected external tool.
    ///
    /// # Information boundary
    ///
    /// The writer exports the active objective and sense, active regular
    /// constraints, and the kinds and bounds of variables used by that model.
    /// Quadratic coefficients follow the convention above.
    /// Unused variables, including those referenced only by output semantics,
    /// named functions, or removed constraints, are omitted. Unused variables
    /// of unsupported kinds do not prevent export.
    ///
    /// ``description.name`` supplies the problem name. Variables and constraints
    /// use generated names ``OMMX_VAR_<id>`` and ``OMMX_CONSTR_<id>``; their
    /// modeling names, subscripts, and other labels are not preserved. Other
    /// Instance information is not exported, including remaining description
    /// fields, annotations, parameters, named functions, removed constraints,
    /// dependencies, fixed-value recovery, and {attr}`~ommx.Instance.output_objective`.
    /// MPS is therefore not a round-trip storage format for an OMMX Instance.
    /// Retain the OMMX instance when its recovery and output semantics are needed
    /// to interpret an external tool's candidates.
    #[pyo3(signature = (path, compress = true))]
    pub fn save_mps(&self, py: Python<'_>, path: String, compress: bool) -> OmmxPyResult<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        ommx::mps::save(&self.inner, path, compress)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load_qplib(py: Python<'_>, path: String) -> OmmxPyResult<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let instance = ommx::qplib::load(path)?;
        Ok(Self { inner: instance })
    }

    /// Generate folded stack format for memory profiling of this instance.
    ///
    /// This method generates a format compatible with flamegraph visualization tools
    /// like ``flamegraph.pl`` and ``inferno``. Each line has the format:
    /// "frame1;frame2;...;frameN bytes"
    ///
    /// The output shows the hierarchical memory structure of the instance, making it
    /// easy to identify which components are consuming the most memory.
    ///
    /// To visualize with flamegraph:
    ///
    /// 1. Save the output to a file: ``profile.txt``
    /// 2. Generate SVG: ``flamegraph.pl profile.txt > memory.svg``
    /// 3. Open memory.svg in a browser
    ///
    /// **Returns:**
    /// Folded stack format string that can be visualized with flamegraph tools
    ///
    /// # Examples
    ///
    /// >>> from ommx import DecisionVariable, Instance, Sense
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints={},
    /// ...     sense=Sense.Maximize,
    /// ... )
    /// >>> profile = instance.logical_memory_profile()
    /// >>> isinstance(profile, str)
    /// True
    pub fn logical_memory_profile(&self) -> String {
        self.inner.logical_memory_profile().to_string()
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct InstanceDescription(pub(crate) ommx::v1::instance::Description);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InstanceDescription {
    #[new]
    #[pyo3(signature = (name = None, description = None, authors = None, created_by = None, created = None, license = None, dataset = None))]
    pub fn new(
        name: Option<String>,
        description: Option<String>,
        authors: Option<Vec<String>>,
        created_by: Option<String>,
        created: Option<String>,
        license: Option<String>,
        dataset: Option<String>,
    ) -> Self {
        let mut desc = ommx::v1::instance::Description::default();
        desc.name = name;
        desc.description = description;
        desc.authors = authors.unwrap_or_default();
        desc.created_by = created_by;
        desc.created = created;
        desc.license = license;
        desc.dataset = dataset;
        Self(desc)
    }
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }

    #[getter]
    pub fn authors(&self) -> Vec<String> {
        self.0.authors.clone()
    }

    #[getter]
    pub fn created_by(&self) -> Option<String> {
        self.0.created_by.clone()
    }

    #[getter]
    pub fn created(&self) -> Option<String> {
        self.0.created.clone()
    }

    #[getter]
    pub fn license(&self) -> Option<String> {
        self.0.license.clone()
    }

    #[getter]
    pub fn dataset(&self) -> Option<String> {
        self.0.dataset.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "InstanceDescription(name={:?}, description={:?}, authors={:?}, created_by={:?}, created={:?}, license={:?}, dataset={:?})",
            self.0.name,
            self.0.description,
            self.0.authors,
            self.0.created_by,
            self.0.created,
            self.0.license,
            self.0.dataset
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
