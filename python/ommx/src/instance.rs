use crate::{
    Constraint, ConstraintHints, DecisionVariable, Function, NamedFunction, ParametricInstance,
    RemovedConstraint, Rng, SampleSet, Samples, Sense, Solution, State, VariableBound,
};
use anyhow::Result;
use ommx::{ConstraintID, Evaluate, NamedFunctionID, Parse, VariableID};
use pyo3::{
    exceptions::PyKeyError,
    prelude::*,
    types::{PyBytes, PyDict},
    Bound, PyAny,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Optimization problem instance.
///
/// Note that this class also contains annotations like :py:attr:`title` which are not contained in protobuf message but stored in OMMX artifact.
/// These annotations are loaded from annotations while reading from OMMX artifact.
///
/// # Examples
/// =========
///
/// Create an instance for KnapSack Problem
///
/// ```python
/// >>> from ommx.v1 import Instance, DecisionVariable
/// ```
///
/// Profit and weight of items
///
/// ```python
/// >>> p = [10, 13, 18, 31, 7, 15]
/// >>> w = [11, 15, 20, 35, 10, 33]
/// ```
///
/// Decision variables
///
/// ```python
/// >>> x = [DecisionVariable.binary(i) for i in range(6)]
/// ```
///
/// Objective and constraint
///
/// ```python
/// >>> objective = sum(p[i] * x[i] for i in range(6))
/// >>> constraint = sum(w[i] * x[i] for i in range(6)) <= 47
/// ```
///
/// Compose as an instance
///
/// ```python
/// >>> instance = Instance.from_components(
/// ...     decision_variables=x,
/// ...     objective=objective,
/// ...     constraints=[constraint],
/// ...     sense=Instance.MAXIMIZE,
/// ... )
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Instance {
    pub(crate) inner: ommx::Instance,
    pub(crate) annotations: HashMap<String, String>,
}

crate::annotations::impl_instance_annotations!(Instance, "org.ommx.v1.instance");

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self {
            inner: ommx::Instance::from_bytes(bytes.as_bytes())?,
            annotations: HashMap::new(),
        })
    }

    /// Create an instance from its components.
    ///
    /// Args:
    /// - sense: Optimization sense (minimize or maximize)
    /// - objective: Objective function
    /// - decision_variables: List of decision variables
    /// - constraints: List of constraints
    /// - named_functions: Optional list of named functions
    /// - description: Optional instance description
    /// - constraint_hints: Optional constraint hints for solvers
    ///
    /// Returns:
    /// A new Instance
    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, named_functions=None, description=None, constraint_hints=None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: Vec<Constraint>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<InstanceDescription>,
        constraint_hints: Option<ConstraintHints>,
    ) -> Result<Self> {
        let mut rust_decision_variables = BTreeMap::new();
        for var in decision_variables {
            let id = var.0.id();
            if rust_decision_variables.insert(id, var.0).is_some() {
                anyhow::bail!("Duplicate decision variable ID: {}", id.into_inner());
            }
        }

        let mut rust_constraints = BTreeMap::new();
        for c in constraints {
            let id = c.0.id;
            if rust_constraints.insert(id, c.0).is_some() {
                anyhow::bail!("Duplicate constraint ID: {}", id.into_inner());
            }
        }

        let mut builder = ommx::Instance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .constraints(rust_constraints);

        if let Some(nfs) = named_functions {
            let mut rust_named_functions = BTreeMap::new();
            for nf in nfs {
                let id = nf.0.id;
                if rust_named_functions.insert(id, nf.0).is_some() {
                    anyhow::bail!("Duplicate named function ID: {}", id.into_inner());
                }
            }
            builder = builder.named_functions(rust_named_functions);
        }

        if let Some(hints) = constraint_hints {
            builder = builder.constraint_hints(hints.0);
        }

        if let Some(desc) = description {
            builder = builder.description(desc.0);
        }

        Ok(Self {
            inner: builder.build()?,
            annotations: HashMap::new(),
        })
    }

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance
    /// >>> instance = Instance.empty()
    /// >>> instance.sense == Instance.MINIMIZE
    /// True
    /// ```
    #[staticmethod]
    pub fn empty() -> Result<Self> {
        Self::from_components(
            Sense::Minimize,
            Function(ommx::Function::Zero),
            Vec::new(),
            Vec::new(),
            None,
            None,
            None,
        )
    }

    #[classattr]
    #[pyo3(name = "MAXIMIZE")]
    fn class_maximize() -> Sense {
        Sense::Maximize
    }

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

    #[getter]
    pub fn sense(&self) -> Sense {
        self.inner.sense().into()
    }

    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.inner.objective().clone())
    }

    #[setter]
    pub fn set_objective(&mut self, objective: Function) -> Result<()> {
        self.inner.set_objective(objective.0)?;
        Ok(())
    }

    /// Get all unique decision variable names in this instance
    #[getter]
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.inner.decision_variable_names()
    }

    /// Get all unique named function names in this instance
    #[getter]
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.inner.named_function_names()
    }

    /// List of all decision variables in the instance sorted by their IDs.
    #[getter]
    pub fn decision_variables(&self) -> Vec<DecisionVariable> {
        self.inner
            .decision_variables()
            .values()
            .map(|var| DecisionVariable(var.clone()))
            .collect()
    }

    /// List of all constraints in the instance sorted by their IDs.
    #[getter]
    pub fn constraints(&self) -> Vec<Constraint> {
        self.inner
            .constraints()
            .values()
            .map(|constraint| Constraint(constraint.clone()))
            .collect()
    }

    /// List of all removed constraints in the instance sorted by their IDs.
    #[getter]
    pub fn removed_constraints(&self) -> Vec<RemovedConstraint> {
        self.inner
            .removed_constraints()
            .values()
            .map(|removed_constraint| RemovedConstraint(removed_constraint.clone()))
            .collect()
    }

    /// List of all named functions in the instance sorted by their IDs.
    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        self.inner
            .named_functions()
            .values()
            .map(|named_function| NamedFunction(named_function.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<InstanceDescription> {
        // Convert Option<v1::instance::Description> to Option<InstanceDescription>
        self.inner
            .description
            .as_ref()
            .map(|desc| InstanceDescription(desc.clone()))
    }

    #[getter]
    pub fn constraint_hints(&self) -> ConstraintHints {
        ConstraintHints(self.inner.constraint_hints().clone())
    }

    #[getter]
    pub fn used_decision_variables(&self) -> Vec<DecisionVariable> {
        self.inner
            .used_decision_variables()
            .values()
            .map(|&var| DecisionVariable(var.clone()))
            .collect()
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let buf = self.inner.to_bytes();
        PyBytes::new(py, &buf)
    }

    /// Get the set of decision variable IDs used in the objective and remaining constraints.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.required_ids()
    /// {0, 1, 2}
    /// ```
    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.inner
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.inner.clone().into();
        let (qubo, constant) = inner.as_qubo_format()?;
        Ok((
            serde_pyobject::to_pyobject(py, &qubo)?
                .extract()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            constant,
        ))
    }

    pub fn as_hubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.inner.clone().into();
        let (hubo, constant) = inner.as_hubo_format()?;
        Ok((
            serde_pyobject::to_pyobject(py, &hubo)?
                .extract()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            constant,
        ))
    }

    /// Convert the instance to a QUBO format.
    ///
    /// This is a **Driver API** for QUBO conversion calling single-purpose methods in order:
    ///
    /// 1. Convert the instance to a minimization problem by :py:meth:`as_minimization_problem`.
    /// 2. Check continuous variables and raise error if exists.
    /// 3. Convert inequality constraints
    ///
    ///   * Try :py:meth:`convert_inequality_to_equality_with_integer_slack` first with given ``inequality_integer_slack_max_range``.
    ///   * If failed, :py:meth:`add_integer_slack_to_inequality`
    ///
    /// 4. Convert to QUBO with (uniform) penalty method
    ///
    ///   * If ``penalty_weights`` is given (in ``dict[constraint_id, weight]`` form), use :py:meth:`penalty_method` with the given weights.
    ///   * If ``uniform_penalty_weight`` is given, use :py:meth:`uniform_penalty_method` with the given weight.
    ///   * If both are None, defaults to ``uniform_penalty_weight = 1.0``.
    ///
    /// 5. Log-encode integer variables by :py:meth:`log_encode`.
    /// 6. Finally convert to QUBO format by :py:meth:`as_qubo_format`.
    ///
    /// Please see the document of each method for details.
    /// If you want to customize the conversion, use the methods above manually.
    ///
    /// # Examples
    /// ========
    ///
    /// Let's consider a maximization problem with two integer variables x0, x1 in [0, 2] subject to an inequality:
    ///
    /// ```text
    /// max  x0 + x1
    /// s.t. x0 + 2*x1 <= 3
    /// ```
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i]) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(x[0] + 2*x[1] <= 3).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// ```
    ///
    /// Convert into QUBO format
    ///
    /// ```python
    /// >>> qubo, offset = instance.to_qubo()
    /// >>> qubo
    /// {(3, 3): -6.0, (3, 4): 2.0, (3, 5): 4.0, (3, 6): 4.0, (3, 7): 2.0, (3, 8): 4.0, (4, 4): -6.0, (4, 5): 4.0, (4, 6): 4.0, (4, 7): 2.0, (4, 8): 4.0, (5, 5): -9.0, (5, 6): 8.0, (5, 7): 4.0, (5, 8): 8.0, (6, 6): -9.0, (6, 7): 4.0, (6, 8): 8.0, (7, 7): -5.0, (7, 8): 4.0, (8, 8): -8.0}
    /// >>> offset
    /// 9.0
    /// ```
    ///
    /// For the maximization problem, the sense is converted to minimization for generating QUBO, and then converted back to maximization.
    ///
    /// ```python
    /// >>> instance.sense == Instance.MAXIMIZE
    /// True
    /// ```
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_qubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<(Bound<'py, PyDict>, f64)> {
        let is_converted = self.as_minimization_problem();
        self.check_no_continuous_variables("QUBO")?;
        self.qubo_hubo_pipeline(
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )?;
        self.log_encode(BTreeSet::new())?;
        let result = self.as_qubo_format(py)?;
        if is_converted {
            self.as_maximization_problem();
        }
        Ok(result)
    }

    /// Convert the instance to a HUBO format.
    ///
    /// This is a **Driver API** for HUBO conversion calling single-purpose methods in order:
    ///
    /// 1. Convert the instance to a minimization problem by :py:meth:`as_minimization_problem`.
    /// 2. Check continuous variables and raise error if exists.
    /// 3. Convert inequality constraints
    ///
    ///   * Try :py:meth:`convert_inequality_to_equality_with_integer_slack` first with given ``inequality_integer_slack_max_range``.
    ///   * If failed, :py:meth:`add_integer_slack_to_inequality`
    ///
    /// 4. Convert to HUBO with (uniform) penalty method
    ///
    ///   * If ``penalty_weights`` is given (in ``dict[constraint_id, weight]`` form), use :py:meth:`penalty_method` with the given weights.
    ///   * If ``uniform_penalty_weight`` is given, use :py:meth:`uniform_penalty_method` with the given weight.
    ///   * If both are None, defaults to ``uniform_penalty_weight = 1.0``.
    ///
    /// 5. Log-encode integer variables by :py:meth:`log_encode`.
    /// 6. Finally convert to HUBO format by :py:meth:`as_hubo_format`.
    ///
    /// Please see the documentation for :py:meth:`to_qubo` for more information, or the
    /// documentation for each individual method for additional details. The
    /// difference between this and :py:meth:`to_qubo` is that this method isn't
    /// restricted to quadratic or linear problems. If you want to customize the
    /// conversion, use the individual methods above manually.
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn to_hubo<'py>(
        &mut self,
        py: Python<'py>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<(Bound<'py, PyDict>, f64)> {
        let is_converted = self.as_minimization_problem();
        self.check_no_continuous_variables("HUBO")?;
        self.qubo_hubo_pipeline(
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )?;
        self.log_encode(BTreeSet::new())?;
        let result = self.as_hubo_format(py)?;
        if is_converted {
            self.as_maximization_problem();
        }
        Ok(result)
    }

    pub fn as_parametric_instance(&self) -> ParametricInstance {
        ParametricInstance {
            inner: self.inner.clone().into(),
            annotations: HashMap::new(),
        }
    }

    /// Convert to a parametric unconstrained instance by penalty method.
    ///
    /// Roughly, this converts a constrained problem:
    ///
    /// ```text
    /// min_x  f(x)
    /// s.t.   g_i(x) = 0   (for all i)
    ///        h_j(x) <= 0  (for all j)
    /// ```
    ///
    /// to an unconstrained problem with parameters:
    ///
    /// ```text
    /// min_x  f(x) + sum_i lambda_i * g_i(x)^2 + sum_j rho_j * h_j(x)^2
    /// ```
    ///
    /// where lambda_i and rho_j are the penalty weight parameters for each constraint.
    /// If you want to use single weight parameter, use :py:meth:`uniform_penalty_method` instead.
    ///
    /// The removed constraints are stored in :py:attr:`~ParametricInstance.removed_constraints`.
    ///
    /// > Note: This method converts inequality constraints h(x) <= 0 to |h(x)|^2 not to max(0, h(x))^2.
    /// > This means the penalty is enforced even for h(x) < 0 cases, and h(x) = 0 is unfairly favored.
    /// > This feature is intended to use with :py:meth:`add_integer_slack_to_inequality`.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Constraint
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[x[0] + x[1] == 1, x[1] + x[2] == 1],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// >>> pi = instance.penalty_method()
    /// ```
    ///
    /// The constraint is put in removed_constraints
    ///
    /// ```python
    /// >>> pi.constraints
    /// []
    /// >>> len(pi.removed_constraints)
    /// 2
    /// >>> pi.removed_constraints[0]
    /// RemovedConstraint(x0 + x1 - 1 == 0, reason=penalty_method, parameter_id=3)
    /// >>> pi.removed_constraints[1]
    /// RemovedConstraint(x1 + x2 - 1 == 0, reason=penalty_method, parameter_id=4)
    /// ```
    pub fn penalty_method(&self) -> Result<ParametricInstance> {
        let parametric_instance = self.inner.clone().penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
            annotations: HashMap::new(),
        })
    }

    /// Convert to a parametric unconstrained instance by penalty method with uniform weight.
    ///
    /// Roughly, this converts a constrained problem:
    ///
    /// ```text
    /// min_x  f(x)
    /// s.t.   g_i(x) = 0   (for all i)
    ///        h_j(x) <= 0  (for all j)
    /// ```
    ///
    /// to an unconstrained problem with a parameter:
    ///
    /// ```text
    /// min_x  f(x) + lambda * (sum_i g_i(x)^2 + sum_j h_j(x)^2)
    /// ```
    ///
    /// where lambda is the uniform penalty weight parameter for all constraints.
    ///
    /// The removed constraints are stored in :py:attr:`~ParametricInstance.removed_constraints`.
    ///
    /// > Note: This method converts inequality constraints h(x) <= 0 to |h(x)|^2 not to max(0, h(x))^2.
    /// > This means the penalty is enforced even for h(x) < 0 cases, and h(x) = 0 is unfairly favored.
    /// > This feature is intended to use with :py:meth:`add_integer_slack_to_inequality`.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[sum(x) == 3],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// >>> pi = instance.uniform_penalty_method()
    /// ```
    ///
    /// The constraint is put in removed_constraints
    ///
    /// ```python
    /// >>> pi.constraints
    /// []
    /// >>> len(pi.removed_constraints)
    /// 1
    /// >>> pi.removed_constraints[0]
    /// RemovedConstraint(x0 + x1 + x2 - 3 == 0, reason=uniform_penalty_method)
    /// ```
    ///
    /// There is only one parameter in the instance
    ///
    /// ```python
    /// >>> len(pi.parameters)
    /// 1
    /// >>> p = pi.parameters[0]
    /// >>> p.id
    /// 3
    /// >>> p.name
    /// 'uniform_penalty_weight'
    /// ```
    pub fn uniform_penalty_method(&self) -> Result<ParametricInstance> {
        let parametric_instance = self.inner.clone().uniform_penalty_method()?;
        Ok(ParametricInstance {
            inner: parametric_instance,
            annotations: HashMap::new(),
        })
    }

    /// Evaluate the given :class:`State` into a :class:`Solution`.
    ///
    /// This method evaluates the problem instance using the provided state (a map from decision variable IDs to their values),
    /// and returns a :class:`Solution` object containing objective value, evaluated constraint values, and feasibility information.
    ///
    /// # Examples
    /// =========
    ///
    /// Create a simple instance with three binary variables and evaluate a solution:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(x[0] + x[1] <= 1).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// ```
    ///
    /// Evaluate it with a state x0 = 1, x1 = 0, x2 = 0, and show the objective and constraints:
    ///
    /// ```python
    /// >>> solution = instance.evaluate({0: 1, 1: 0, 2: 0})
    /// >>> solution.objective
    /// 1.0
    /// ```
    ///
    /// If the value is out of the range, the solution is infeasible:
    ///
    /// ```python
    /// >>> solution = instance.evaluate({0: 1, 1: 0, 2: 2})
    /// >>> solution.feasible
    /// False
    /// ```
    ///
    /// If some of the decision variables are not set, this raises an error:
    ///
    /// ```python
    /// >>> instance.evaluate({0: 1, 1: 0})
    /// ```
    /// Traceback (most recent call last):
    ///     ...
    /// ValueError: The state does not contain some required IDs: {VariableID(2)}
    #[pyo3(signature = (state, *, atol=None))]
    pub fn evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Solution> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let solution = self
            .inner
            .evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Solution {
            inner: solution,
            annotations: HashMap::new(),
        })
    }

    /// Creates a new instance with specific decision variables fixed to given values.
    ///
    /// This method substitutes the specified decision variables with their provided values,
    /// creating a new problem instance where these variables are fixed. This is useful for
    /// scenarios such as:
    ///
    /// - Creating simplified sub-problems with some variables fixed
    /// - Incrementally solving a problem by fixing some variables and optimizing the rest
    /// - Testing specific configurations of a problem
    ///
    /// Args:
    /// - state: Maps decision variable IDs to their fixed values.
    ///   Can be a :class:`~ommx.v1.State` object or a dictionary mapping variable IDs to values.
    /// - atol: Absolute tolerance for floating point comparisons. If None, uses the default tolerance.
    ///
    /// Returns:
    /// A new instance with the specified decision variables fixed to their given values.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = DecisionVariable.binary(1)
    /// >>> y = DecisionVariable.binary(2)
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=[x, y],
    /// ...     objective=x + y,
    /// ...     constraints=[x + y <= 1],
    /// ...     sense=Instance.MINIMIZE
    /// ... )
    /// >>> new_instance = instance.partial_evaluate({1: 1})
    /// >>> new_instance.objective
    /// Function(x2 + 1)
    /// ```
    ///
    /// Substituted value is stored in the decision variable:
    ///
    /// ```python
    /// >>> x = new_instance.get_decision_variable_by_id(1)
    /// >>> x.substituted_value
    /// 1.0
    /// ```
    #[pyo3(signature = (state, *, atol=None))]
    pub fn partial_evaluate(&self, state: State, atol: Option<f64>) -> PyResult<Self> {
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => ommx::ATol::default(),
        };
        let mut new_inner = self.inner.clone();
        new_inner
            .partial_evaluate(&state.0, atol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner: new_inner,
            annotations: self.annotations.clone(),
        })
    }

    #[pyo3(signature = (samples, *, atol=None))]
    pub fn evaluate_samples(&self, samples: Samples, atol: Option<f64>) -> Result<SampleSet> {
        let v1_samples: ommx::v1::Samples = samples.0.into();
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        Ok(SampleSet {
            inner: self.inner.evaluate_samples(&v1_samples, atol)?,
            annotations: HashMap::new(),
        })
    }

    /// Generate a random state for this instance using the provided random number generator.
    ///
    /// This method generates random values only for variables that are actually used in the
    /// objective function or constraints, as determined by decision variable analysis.
    /// Generated values respect the bounds of each variable type.
    ///
    /// Args:
    /// - rng: Random number generator to use for generating the state.
    ///
    /// Returns:
    /// A randomly generated state that satisfies the variable bounds of this instance.
    /// Only contains values for variables that are used in the problem.
    ///
    /// # Examples
    /// =========
    ///
    /// Generate random state only for used variables
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Rng
    /// >>> x = [DecisionVariable.binary(i) for i in range(5)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    ///
    /// >>> rng = Rng()
    /// >>> state = instance.random_state(rng)
    /// ```
    ///
    /// Only used variables have values
    ///
    /// ```python
    /// >>> set(state.entries.keys())
    /// {0, 1}
    /// ```
    ///
    /// Values respect binary bounds
    ///
    /// ```python
    /// >>> all(state.entries[i] in [0.0, 1.0] for i in state.entries)
    /// True
    /// ```
    pub fn random_state(&self, rng: &Rng) -> Result<crate::State> {
        let strategy = self.inner.arbitrary_state();
        let mut rng_guard = rng
            .lock()
            .map_err(|_| anyhow::anyhow!("Cannot get lock for RNG"))?;
        let state = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::State(state))
    }

    /// Generate random samples for this instance.
    ///
    /// The generated samples will contain ``num_samples`` sample entries divided into
    /// ``num_different_samples`` groups, where each group shares the same state but has
    /// different sample IDs.
    ///
    /// Args:
    /// - rng: Random number generator
    /// - num_different_samples: Number of different states to generate
    /// - num_samples: Total number of samples to generate
    /// - max_sample_id: Maximum sample ID (default: ``num_samples``)
    ///
    /// Returns:
    /// Samples object
    ///
    /// # Examples
    /// ========
    ///
    /// Generate samples for a simple instance:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable, Rng
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(sum(x) <= 2).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    ///
    /// >>> rng = Rng()
    /// >>> samples = instance.random_samples(rng, num_different_samples=2, num_samples=5)
    /// >>> samples.num_samples()
    /// 5
    /// ```
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
    ) -> Result<crate::Samples> {
        let max_sample_id = max_sample_id.unwrap_or(num_samples as u64);
        let params = ommx::random::SamplesParameters::new(
            num_different_samples,
            num_samples,
            max_sample_id,
        )?;

        let strategy = self.inner.arbitrary_samples(params);
        let mut rng_guard = rng
            .lock()
            .map_err(|_| anyhow::anyhow!("Cannot get lock for RNG"))?;
        let samples = ommx::random::sample(&mut rng_guard, strategy);
        Ok(crate::Samples(samples))
    }

    /// Remove a constraint from the instance.
    ///
    /// The removed constraint is stored in :py:attr:`~Instance.removed_constraints`, and can be restored by :py:meth:`restore_constraint`.
    ///
    /// Args:
    /// - constraint_id: The ID of the constraint to remove.
    /// - reason: The reason why the constraint is removed.
    /// - parameters: Additional parameters to describe the reason.
    ///
    /// # Examples
    /// =========
    ///
    /// Relax constraint, and restore it.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[(sum(x) == 3).set_id(1)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.constraints
    /// [Constraint(x0 + x1 + x2 - 3 == 0)]
    /// ```
    ///
    /// ```python
    /// >>> instance.relax_constraint(1, "manual relaxation")
    /// >>> instance.constraints
    /// []
    /// >>> instance.removed_constraints
    /// [RemovedConstraint(x0 + x1 + x2 - 3 == 0, reason=manual relaxation)]
    /// ```
    ///
    /// ```python
    /// >>> instance.restore_constraint(1)
    /// >>> instance.constraints
    /// [Constraint(x0 + x1 + x2 - 3 == 0)]
    /// >>> instance.removed_constraints
    /// []
    /// ```
    #[pyo3(signature = (constraint_id, reason, **parameters))]
    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        reason: String,
        #[gen_stub(override_type(type_repr = "str"))] parameters: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.inner.relax_constraint(
            constraint_id.into(),
            reason,
            parameters.unwrap_or_default(),
        )?;
        Ok(())
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.inner.restore_constraint(constraint_id.into())?;
        Ok(())
    }

    /// Log-encode the integer decision variables.
    ///
    /// Log encoding of an integer variable x in [l, u] is to represent by m bits b_i in {0, 1} by:
    ///
    /// ```text
    /// x = sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l
    /// ```
    ///
    /// where m = ceil(log2(u - l + 1)).
    ///
    /// Args:
    /// - decision_variable_ids: The IDs of the integer decision variables to log-encode.
    ///   If not specified (or empty), all integer variables are log-encoded.
    ///
    /// # Examples
    /// =========
    ///
    /// Let's consider a simple integer programming problem with three integer variables x0, x1, and x2.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// ```
    ///
    /// To log-encode the integer variables x0 and x2 (except x1), call log_encode:
    ///
    /// ```python
    /// >>> instance.log_encode({0, 2})
    /// ```
    ///
    /// Integer variable in range [0, 3] can be represented by two binary variables:
    ///
    /// x0 = b_{0,0} + 2 b_{0,1}, x2 = b_{2,0} + 2 b_{2,1}
    ///
    /// And these are substituted into the objective and constraint functions.
    ///
    /// ```python
    /// >>> instance.objective
    /// Function(x1 + x3 + 2*x4 + x5 + 2*x6)
    /// ```
    #[pyo3(signature = (decision_variable_ids=BTreeSet::new()))]
    pub fn log_encode(&mut self, decision_variable_ids: BTreeSet<u64>) -> Result<()> {
        let ids: BTreeSet<u64> = if decision_variable_ids.is_empty() {
            // Auto-detect: find all used integer decision variables
            let analysis = self.inner.analyze_decision_variables();
            let integer_ids: BTreeSet<u64> = analysis
                .used_integer()
                .into_iter()
                .map(|(id, _)| id.into_inner())
                .collect();
            if integer_ids.is_empty() {
                return Ok(());
            }
            integer_ids
        } else {
            decision_variable_ids
        };
        for id in ids.iter() {
            self.inner.log_encode((*id).into())?;
        }
        Ok(())
    }

    /// Convert an inequality constraint f(x) <= 0 to an equality constraint f(x) + s/a = 0 with an integer slack variable s.
    ///
    /// - Since a is determined as the minimal multiplier to make every coefficient of af(x) integer,
    ///   a itself and the range of s becomes impractically large. ``max_integer_range`` limits the maximal
    ///   range of s, and returns error if the range exceeds it.
    ///
    /// - Since this method evaluates the bound of f(x), we may find that:
    ///
    ///   - The bound [l, u] is strictly positive, i.e. l > 0:
    ///     this means the instance is infeasible because this constraint never be satisfied,
    ///     and an error is raised.
    ///
    ///   - The bound [l, u] is always negative, i.e. u <= 0:
    ///     this means this constraint is trivially satisfied,
    ///     the constraint is moved to :py:attr:`~Instance.removed_constraints`,
    ///     and this method returns without introducing slack variable or raising an error.
    ///
    /// # Examples
    /// =========
    ///
    /// Let's consider a simple inequality constraint x0 + 2*x1 <= 5.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[
    /// ...         (x[0] + 2*x[1] <= 5).set_id(0)
    /// ...     ],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.constraints[0]
    /// Constraint(x0 + 2*x1 - 5 <= 0)
    /// ```
    ///
    /// Introduce an integer slack variable
    ///
    /// ```python
    /// >>> instance.convert_inequality_to_equality_with_integer_slack(
    /// ...     constraint_id=0,
    /// ...     max_integer_range=32
    /// ... )
    /// >>> instance.constraints[0]
    /// Constraint(x0 + 2*x1 + x3 - 5 == 0)
    /// ```
    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
    ) -> Result<()> {
        let mut inner: ommx::v1::Instance = self.inner.clone().into();
        inner.convert_inequality_to_equality_with_integer_slack(
            constraint_id,
            max_integer_range,
            ommx::ATol::default(),
        )?;
        self.inner = Parse::parse(inner, &())?;
        Ok(())
    }

    /// Convert inequality f(x) <= 0 to **inequality** f(x) + b*s <= 0 with an integer slack variable s.
    ///
    /// - This should be used when :meth:`convert_inequality_to_equality_with_integer_slack` is not applicable.
    ///
    /// - The bound of s will be [0, slack_upper_bound], and the coefficient b is determined from the lower bound of f(x).
    ///
    /// - Since the slack variable is integer, the yielded inequality has residual error min_s f(x) + b*s at most b.
    ///   And thus b is returned to use scaling the penalty weight or other things.
    ///
    ///   - Larger slack_upper_bound (i.e. fined-grained slack) yields smaller b, and thus smaller the residual error,
    ///     but it needs more bits for the slack variable, and thus the problem size becomes larger.
    ///
    /// Returns:
    /// The coefficient b of the slack variable. If the constraint is trivially satisfied, this returns ``None``.
    ///
    /// # Examples
    /// =========
    ///
    /// Let's consider a simple inequality constraint x0 + 2*x1 <= 4.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [
    /// ...     DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    /// ...     for i in range(3)
    /// ... ]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[
    /// ...         (x[0] + 2*x[1] <= 4).set_id(0)
    /// ...     ],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.constraints[0]
    /// Constraint(x0 + 2*x1 - 4 <= 0)
    /// ```
    ///
    /// Introduce an integer slack variable s in [0, 2]
    ///
    /// ```python
    /// >>> b = instance.add_integer_slack_to_inequality(
    /// ...     constraint_id=0,
    /// ...     slack_upper_bound=2
    /// ... )
    /// >>> b, instance.constraints[0]
    /// (2.0, Constraint(x0 + 2*x1 + 2*x3 - 4 <= 0))
    /// ```
    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        let mut inner: ommx::v1::Instance = self.inner.clone().into();
        let result = inner.add_integer_slack_to_inequality(constraint_id, slack_upper_bound)?;
        self.inner = Parse::parse(inner, &())?;
        Ok(result)
    }

    /// Analyze decision variables in the optimization problem instance.
    ///
    /// Returns a comprehensive analysis of all decision variables including:
    ///
    /// - Kind-based partitioning (binary, integer, continuous, etc.)
    /// - Usage-based partitioning (used in objective, constraints, fixed, etc.)
    /// - Variable bounds information
    ///
    /// Returns:
    /// Analysis object containing detailed information about decision variables
    ///
    /// # Examples
    /// --------
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints=[(x[1] + x[2] == 1).set_id(0)],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> analysis = instance.decision_variable_analysis()
    /// >>> analysis.used_decision_variable_ids()
    /// {0, 1, 2}
    /// >>> analysis.used_in_objective()
    /// {0, 1}
    /// >>> analysis.used_in_constraints()
    /// {0: {1, 2}}
    /// ```
    pub fn decision_variable_analysis(&self) -> DecisionVariableAnalysis {
        DecisionVariableAnalysis(self.inner.analyze_decision_variables())
    }

    /// Get statistics about the instance.
    ///
    /// Returns a dictionary containing counts of decision variables and constraints
    /// categorized by kind, usage, and status.
    ///
    /// Returns:
    /// A dictionary with the following structure:
    ///
    /// ```json
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
    /// --------
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance
    /// >>> instance = Instance.empty()
    /// >>> stats = instance.stats()
    /// >>> stats["decision_variables"]["total"]
    /// 0
    /// >>> stats["constraints"]["total"]
    /// 0
    /// ```
    pub fn stats<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let stats = self.inner.stats();
        serde_pyobject::to_pyobject(py, &stats)?
            .extract()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// DataFrame of decision variables
    #[gen_stub(override_return_type(type_repr = "pandas.DataFrame", imports = ("pandas",)))]
    #[getter]
    pub fn decision_variables_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let na = pandas.getattr("NA")?;
        let entries: Vec<_> = self
            .inner
            .decision_variables()
            .values()
            .map(|v| DecisionVariable(v.clone()).as_pandas_entry(py, &na))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    /// DataFrame of constraints
    #[gen_stub(override_return_type(type_repr = "pandas.DataFrame", imports = ("pandas",)))]
    #[getter]
    pub fn constraints_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let entries: Vec<_> = self
            .inner
            .constraints()
            .values()
            .map(|c| Constraint(c.clone())._as_pandas_entry(py))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    /// DataFrame of removed constraints
    #[gen_stub(override_return_type(type_repr = "pandas.DataFrame", imports = ("pandas",)))]
    #[getter]
    pub fn removed_constraints_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let entries: Vec<_> = self
            .inner
            .removed_constraints()
            .values()
            .map(|rc| RemovedConstraint(rc.clone())._as_pandas_entry(py))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    /// DataFrame of named functions
    #[gen_stub(override_return_type(type_repr = "pandas.DataFrame", imports = ("pandas",)))]
    #[getter]
    pub fn named_functions_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pandas = py.import("pandas")?;
        let entries: Vec<_> = self
            .inner
            .named_functions()
            .values()
            .map(|nf| NamedFunction(nf.clone())._as_pandas_entry(py))
            .collect::<PyResult<_>>()?;
        let df = pandas.call_method1("DataFrame", (entries,))?;
        if df.getattr("empty")?.extract::<bool>()? {
            return Ok(df);
        }
        df.call_method1("set_index", ("id",))
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Convert the instance to a minimization problem.
    ///
    /// If the instance is already a minimization problem, this does nothing.
    ///
    /// Returns:
    /// ``True`` if the instance is converted, ``False`` if already a minimization problem.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[sum(x) == 1],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> instance.sense == Instance.MAXIMIZE
    /// True
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// ```
    ///
    /// Convert to a minimization problem
    ///
    /// ```python
    /// >>> instance.as_minimization_problem()
    /// True
    /// >>> instance.sense == Instance.MINIMIZE
    /// True
    /// >>> instance.objective
    /// Function(-x0 - x1 - x2)
    /// ```
    ///
    /// If the instance is already a minimization problem, this does nothing
    ///
    /// ```python
    /// >>> instance.as_minimization_problem()
    /// False
    /// ```
    pub fn as_minimization_problem(&mut self) -> bool {
        self.inner.as_minimization_problem()
    }

    /// Convert the instance to a maximization problem.
    ///
    /// If the instance is already a maximization problem, this does nothing.
    ///
    /// Returns:
    /// ``True`` if the instance is converted, ``False`` if already a maximization problem.
    ///
    /// # Examples
    /// =========
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=sum(x),
    /// ...     constraints=[sum(x) == 1],
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.sense == Instance.MINIMIZE
    /// True
    /// >>> instance.objective
    /// Function(x0 + x1 + x2)
    /// ```
    ///
    /// Convert to a maximization problem
    ///
    /// ```python
    /// >>> instance.as_maximization_problem()
    /// True
    /// >>> instance.sense == Instance.MAXIMIZE
    /// True
    /// >>> instance.objective
    /// Function(-x0 - x1 - x2)
    /// ```
    ///
    /// If the instance is already a maximization problem, this does nothing
    ///
    /// ```python
    /// >>> instance.as_maximization_problem()
    /// False
    /// ```
    pub fn as_maximization_problem(&mut self) -> bool {
        self.inner.as_maximization_problem()
    }

    /// Get a specific decision variable by ID
    pub fn get_decision_variable_by_id(&self, variable_id: u64) -> PyResult<DecisionVariable> {
        self.inner
            .decision_variables()
            .get(&VariableID::from(variable_id))
            .map(|var| DecisionVariable(var.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific constraint by ID
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<Constraint> {
        self.inner
            .constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|constraint| Constraint(constraint.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Constraint with ID {constraint_id} not found"))
            })
    }

    /// Get a specific removed constraint by ID
    pub fn get_removed_constraint_by_id(&self, constraint_id: u64) -> PyResult<RemovedConstraint> {
        self.inner
            .removed_constraints()
            .get(&ConstraintID::from(constraint_id))
            .map(|removed_constraint| RemovedConstraint(removed_constraint.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Removed constraint with ID {constraint_id} not found"
                ))
            })
    }

    /// Get a specific named function by ID
    pub fn get_named_function_by_id(&self, named_function_id: u64) -> PyResult<NamedFunction> {
        self.inner
            .named_functions()
            .get(&NamedFunctionID::from(named_function_id))
            .map(|named_function| NamedFunction(named_function.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Named function with ID {named_function_id} not found"
                ))
            })
    }

    /// Reduce binary powers in the instance.
    ///
    /// This method replaces binary powers in the instance with their equivalent linear expressions.
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns:
    /// ``True`` if any reduction was performed, ``False`` otherwise.
    ///
    /// # Examples
    /// =========
    ///
    /// Consider an instance with binary variables and quadratic terms:
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(2)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] * x[0] + x[0] * x[1],
    /// ...     constraints=[],
    /// ...     sense=Instance.MINIMIZE,
    /// ... )
    /// >>> instance.objective
    /// Function(x0*x0 + x0*x1)
    /// ```
    ///
    /// After reducing binary powers, x0^2 becomes x0:
    ///
    /// ```python
    /// >>> changed = instance.reduce_binary_power()
    /// >>> changed
    /// True
    /// >>> instance.objective
    /// Function(x0*x1 + x0)
    /// ```
    ///
    /// Running it again should not change anything:
    ///
    /// ```python
    /// >>> changed = instance.reduce_binary_power()
    /// >>> changed
    /// False
    /// ```
    pub fn reduce_binary_power(&mut self) -> bool {
        self.inner.reduce_binary_power()
    }

    #[staticmethod]
    pub fn load_mps(path: String) -> Result<Self> {
        let instance = ommx::mps::load(path)?;
        Ok(Self {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    #[pyo3(signature = (path, compress = true))]
    pub fn save_mps(&self, path: String, compress: bool) -> Result<()> {
        ommx::mps::save(&self.inner, path, compress)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load_qplib(path: String) -> Result<Self> {
        let instance = ommx::qplib::load(path)?;
        Ok(Self {
            inner: instance,
            annotations: HashMap::new(),
        })
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
    /// Returns:
    /// Folded stack format string that can be visualized with flamegraph tools
    ///
    /// # Examples
    /// --------
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance, DecisionVariable
    /// >>> x = [DecisionVariable.binary(i) for i in range(3)]
    /// >>> instance = Instance.from_components(
    /// ...     decision_variables=x,
    /// ...     objective=x[0] + x[1],
    /// ...     constraints=[],
    /// ...     sense=Instance.MAXIMIZE,
    /// ... )
    /// >>> profile = instance.logical_memory_profile()
    /// >>> isinstance(profile, str)
    /// True
    /// ```
    pub fn logical_memory_profile(&self) -> String {
        ommx::logical_memory::logical_memory_to_folded(&self.inner)
    }
}

impl Instance {
    pub(crate) fn check_no_continuous_variables(&self, format_name: &str) -> Result<()> {
        let continuous_ids: Vec<u64> = self
            .inner
            .analyze_decision_variables()
            .used_continuous()
            .into_iter()
            .map(|(id, _)| id.into_inner())
            .collect();
        if !continuous_ids.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Continuous variables are not supported in {} conversion: IDs={:?}",
                format_name, continuous_ids
            ))
            .into());
        }
        Ok(())
    }

    /// Shared pipeline for to_qubo/to_hubo: handle inequality constraints, apply penalty method.
    pub(crate) fn qubo_hubo_pipeline(
        &mut self,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<HashMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> Result<()> {
        // Prepare inequality constraints
        let ineq_ids: Vec<ConstraintID> = self
            .inner
            .constraints()
            .iter()
            .filter(|(_, c)| c.equality == ommx::Equality::LessThanOrEqualToZero)
            .map(|(id, _)| *id)
            .collect();
        for ineq_id in ineq_ids {
            let id_u64 = ineq_id.into_inner();
            // Try exact integer slack first, fall back to approximate
            if self
                .convert_inequality_to_equality_with_integer_slack(
                    id_u64,
                    inequality_integer_slack_max_range,
                )
                .is_err()
            {
                self.add_integer_slack_to_inequality(id_u64, inequality_integer_slack_max_range)?;
            }
        }

        // Penalty method
        if !self.inner.constraints().is_empty() {
            if uniform_penalty_weight.is_some() && penalty_weights.is_some() {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Both uniform_penalty_weight and penalty_weights are specified. Please choose one."
                ).into());
            }
            if let Some(pw) = penalty_weights {
                let pi = self.inner.clone().penalty_method()?;
                // Map constraint IDs (from parameter subscripts) to penalty weights
                let mut weights = HashMap::new();
                for p in pi.parameters().values() {
                    let constraint_id = p.subscripts.first().copied().ok_or_else(|| {
                        anyhow::anyhow!("Penalty parameter {} has no subscripts", p.id)
                    })? as u64;
                    let w = pw.get(&constraint_id).ok_or_else(|| {
                        anyhow::anyhow!(
                            "No penalty weight provided for constraint ID {}",
                            constraint_id
                        )
                    })?;
                    weights.insert(VariableID::from(p.id).into_inner(), *w);
                }
                let mut v1_params = ommx::v1::Parameters::default();
                v1_params.entries = weights;
                self.inner = pi.with_parameters(v1_params)?;
            } else {
                let weight = uniform_penalty_weight.unwrap_or(1.0);
                let pi = self.inner.clone().uniform_penalty_method()?;
                let param_id = pi
                    .parameters()
                    .keys()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("No penalty weight parameter found"))?;
                let mut v1_params = ommx::v1::Parameters::default();
                v1_params.entries.insert(param_id.into_inner(), weight);
                self.inner = pi.with_parameters(v1_params)?;
            }
        }

        Ok(())
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct DecisionVariableAnalysis(ommx::DecisionVariableAnalysis);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl DecisionVariableAnalysis {
    pub fn used_binary(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_binary()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_integer(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_integer()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_continuous(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_continuous()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_semi_integer(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_semi_integer()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_semi_continuous(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_semi_continuous()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0.used().iter().map(|id| id.into_inner()).collect()
    }

    pub fn all_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0.all().iter().map(|id| id.into_inner()).collect()
    }

    pub fn used_in_objective(&self) -> BTreeSet<u64> {
        self.0
            .used_in_objective()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn used_in_constraints(&self) -> BTreeMap<u64, BTreeSet<u64>> {
        self.0
            .used_in_constraints()
            .iter()
            .map(|(constraint_id, variable_ids)| {
                (
                    **constraint_id,
                    variable_ids.iter().map(|id| id.into_inner()).collect(),
                )
            })
            .collect()
    }

    pub fn fixed(&self) -> BTreeMap<u64, f64> {
        self.0
            .fixed()
            .iter()
            .map(|(id, value)| (id.into_inner(), *value))
            .collect()
    }

    pub fn irrelevant(&self) -> BTreeSet<u64> {
        self.0
            .irrelevant()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn dependent(&self) -> BTreeSet<u64> {
        self.0
            .dependent()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let obj = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(obj.cast::<PyDict>()?.clone())
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
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
    #[pyo3(signature = (name = None, description = None, authors = None, created_by = None))]
    pub fn new(
        name: Option<String>,
        description: Option<String>,
        authors: Option<Vec<String>>,
        created_by: Option<String>,
    ) -> Self {
        let mut desc = ommx::v1::instance::Description::default();
        desc.name = name;
        desc.description = description;
        desc.authors = authors.unwrap_or_default();
        desc.created_by = created_by;
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

    fn __repr__(&self) -> String {
        format!(
            "InstanceDescription(name={:?}, description={:?}, authors={:?}, created_by={:?})",
            self.0.name, self.0.description, self.0.authors, self.0.created_by
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
