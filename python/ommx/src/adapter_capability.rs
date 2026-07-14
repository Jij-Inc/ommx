use crate::{Equality, Kind, Sense};
use pyo3::{exceptions::PyValueError, prelude::*};
use std::collections::{BTreeMap, BTreeSet};

fn capability_definition_error(error: ommx::CapabilityDefinitionError) -> PyErr {
    PyValueError::new_err(error.to_string())
}

/// Cumulative polynomial-degree support in a capability profile.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegreeLimit(pub ommx::DegreeLimit);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl DegreeLimit {
    /// Accept every degree up to and including ``maximum``.
    #[staticmethod]
    pub fn at_most(maximum: u32) -> Self {
        Self(ommx::DegreeLimit::at_most(maximum))
    }

    /// Accept any polynomial degree representable by OMMX.
    #[staticmethod]
    pub fn any() -> Self {
        Self(ommx::DegreeLimit::Any)
    }

    /// Inclusive maximum degree, or ``None`` for :meth:`any`.
    #[getter]
    pub fn maximum(&self) -> Option<u32> {
        self.0.maximum().map(|degree| degree.into_inner())
    }

    /// Return whether ``actual_degree`` is accepted.
    pub fn allows(&self, actual_degree: u32) -> bool {
        self.0.allows(actual_degree.into())
    }

    pub fn __repr__(&self) -> String {
        match self.maximum() {
            Some(maximum) => format!("DegreeLimit.at_most({maximum})"),
            None => "DegreeLimit.any()".to_string(),
        }
    }
}

/// Relation and polynomial degree required by one active constraint.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintRequirement(pub ommx::ConstraintRequirement);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintRequirement {
    #[getter]
    pub fn relation(&self) -> Equality {
        self.0.relation().into()
    }

    #[getter]
    pub fn degree(&self) -> u32 {
        self.0.degree().into_inner()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ConstraintRequirement(relation={:?}, degree={})",
            self.relation(),
            self.degree()
        )
    }
}

/// Portable shape of an instance's complete active solver input.
///
/// Fixed, dependent, irrelevant, removed-constraint-only, and
/// named-function-only variables are excluded.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRequirements(pub ommx::InstanceRequirements);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InstanceRequirements {
    #[getter]
    pub fn sense(&self) -> Sense {
        self.0.sense().into()
    }

    #[getter]
    pub fn used_variables_by_kind(&self) -> BTreeMap<Kind, BTreeSet<u64>> {
        self.0
            .used_variables_by_kind()
            .iter()
            .map(|(kind, ids)| {
                (
                    (*kind).into(),
                    ids.iter().map(|id| id.into_inner()).collect(),
                )
            })
            .collect()
    }

    #[getter]
    pub fn used_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .used_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    #[getter]
    pub fn objective_degree(&self) -> u32 {
        self.0.objective_degree().into_inner()
    }

    #[getter]
    pub fn regular_constraints(&self) -> BTreeMap<u64, ConstraintRequirement> {
        self.0
            .regular_constraints()
            .iter()
            .map(|(id, requirement)| ((*id).into_inner(), ConstraintRequirement(*requirement)))
            .collect()
    }

    #[getter]
    pub fn indicator_constraints(&self) -> BTreeMap<u64, ConstraintRequirement> {
        self.0
            .indicator_constraints()
            .iter()
            .map(|(id, requirement)| ((*id).into_inner(), ConstraintRequirement(*requirement)))
            .collect()
    }

    #[getter]
    pub fn one_hot_constraint_ids(&self) -> BTreeSet<u64> {
        self.0
            .one_hot_constraint_ids()
            .iter()
            .map(|id| (*id).into_inner())
            .collect()
    }

    #[getter]
    pub fn sos1_constraint_ids(&self) -> BTreeSet<u64> {
        self.0
            .sos1_constraint_ids()
            .iter()
            .map(|id| (*id).into_inner())
            .collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "InstanceRequirements(sense={:?}, objective_degree={}, used_variable_ids={:?})",
            self.sense(),
            self.objective_degree(),
            self.used_variable_ids()
        )
    }
}

/// One coherent combination of native solver capabilities.
///
/// This describes direct translator input after any explicit preparation.
/// Exact reformulation, relaxation, and heuristic or finite-penalty conversion
/// are preparation concerns rather than native capabilities.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile(pub ommx::CapabilityProfile);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl CapabilityProfile {
    #[new]
    #[pyo3(signature = (*, name, variable_kinds, objective_degree, senses, regular_constraints=None, indicator_constraints=None, supports_one_hot=false, supports_sos1=false))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        variable_kinds: BTreeSet<Kind>,
        objective_degree: DegreeLimit,
        senses: BTreeSet<Sense>,
        regular_constraints: Option<BTreeMap<Equality, DegreeLimit>>,
        indicator_constraints: Option<BTreeMap<Equality, DegreeLimit>>,
        supports_one_hot: bool,
        supports_sos1: bool,
    ) -> PyResult<Self> {
        let mut profile = ommx::CapabilityProfile::new(
            name,
            variable_kinds.into_iter().map(Into::into).collect(),
            objective_degree.0,
            senses.into_iter().map(Into::into).collect(),
        )
        .map_err(capability_definition_error)?;
        for (relation, limit) in regular_constraints.unwrap_or_default() {
            profile = profile.with_regular_constraint(relation.into(), limit.0);
        }
        for (relation, limit) in indicator_constraints.unwrap_or_default() {
            profile = profile.with_indicator_constraint(relation.into(), limit.0);
        }
        if supports_one_hot {
            profile = profile.with_one_hot();
        }
        if supports_sos1 {
            profile = profile.with_sos1();
        }
        Ok(Self(profile))
    }

    #[getter]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    #[getter]
    pub fn variable_kinds(&self) -> BTreeSet<Kind> {
        self.0
            .variable_kinds()
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    pub fn objective_degree(&self) -> DegreeLimit {
        DegreeLimit(self.0.objective_degree())
    }

    #[getter]
    pub fn regular_constraints(&self) -> BTreeMap<Equality, DegreeLimit> {
        self.0
            .regular_constraints()
            .iter()
            .map(|(relation, limit)| (relation.into(), DegreeLimit(limit)))
            .collect()
    }

    #[getter]
    pub fn indicator_constraints(&self) -> BTreeMap<Equality, DegreeLimit> {
        self.0
            .indicator_constraints()
            .iter()
            .map(|(relation, limit)| (relation.into(), DegreeLimit(limit)))
            .collect()
    }

    #[getter]
    pub fn supports_one_hot(&self) -> bool {
        self.0.supports_one_hot()
    }

    #[getter]
    pub fn supports_sos1(&self) -> bool {
        self.0.supports_sos1()
    }

    #[getter]
    pub fn senses(&self) -> BTreeSet<Sense> {
        self.0.senses().iter().copied().map(Into::into).collect()
    }

    pub fn __repr__(&self) -> String {
        format!("CapabilityProfile(name={:?})", self.name())
    }
}

/// Validated alternative native capability profiles for an adapter.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterCapabilities(pub ommx::AdapterCapabilities);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl AdapterCapabilities {
    #[new]
    pub fn new(profiles: Vec<CapabilityProfile>) -> PyResult<Self> {
        ommx::AdapterCapabilities::new(profiles.into_iter().map(|profile| profile.0).collect())
            .map(Self)
            .map_err(capability_definition_error)
    }

    #[getter]
    pub fn profiles(&self) -> Vec<CapabilityProfile> {
        self.0
            .profiles()
            .iter()
            .cloned()
            .map(CapabilityProfile)
            .collect()
    }

    /// Compare native profiles without mutating or preparing the input.
    pub fn check_compatibility(
        &self,
        requirements: &InstanceRequirements,
    ) -> PortableCompatibilityReport {
        PortableCompatibilityReport(self.0.check_compatibility(&requirements.0))
    }

    pub fn __repr__(&self) -> String {
        format!("AdapterCapabilities(profiles={:?})", self.profiles())
    }
}

/// One structured incompatibility between requirements and a complete profile.
#[pyo3_stub_gen::derive::gen_stub_pyclass_complex_enum]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableCapabilityMismatch {
    UnsupportedVariableKind {
        kind: Kind,
        used_variable_ids: BTreeSet<u64>,
        supported_kinds: BTreeSet<Kind>,
    },
    ObjectiveDegreeExceeded {
        actual_degree: u32,
        limit: DegreeLimit,
    },
    UnsupportedRegularConstraintRelation {
        relation: Equality,
        constraint_ids: BTreeSet<u64>,
        supported_relations: BTreeSet<Equality>,
    },
    RegularConstraintDegreeExceeded {
        relation: Equality,
        actual_degrees: BTreeMap<u64, u32>,
        limit: DegreeLimit,
    },
    UnsupportedIndicatorConstraints {
        constraint_ids: BTreeSet<u64>,
    },
    UnsupportedIndicatorConstraintRelation {
        relation: Equality,
        constraint_ids: BTreeSet<u64>,
        supported_relations: BTreeSet<Equality>,
    },
    IndicatorBodyDegreeExceeded {
        relation: Equality,
        actual_degrees: BTreeMap<u64, u32>,
        limit: DegreeLimit,
    },
    UnsupportedOneHotConstraints {
        constraint_ids: BTreeSet<u64>,
    },
    UnsupportedSos1Constraints {
        constraint_ids: BTreeSet<u64>,
    },
    UnsupportedSense {
        sense: Sense,
        supported_senses: BTreeSet<Sense>,
    },
    Unknown {
        message: String,
    },
}

impl From<ommx::PortableCapabilityMismatch> for PortableCapabilityMismatch {
    fn from(mismatch: ommx::PortableCapabilityMismatch) -> Self {
        match mismatch {
            ommx::PortableCapabilityMismatch::UnsupportedVariableKind {
                kind,
                used_variable_ids,
                supported_kinds,
            } => Self::UnsupportedVariableKind {
                kind: kind.into(),
                used_variable_ids: used_variable_ids.iter().map(|id| id.into_inner()).collect(),
                supported_kinds: supported_kinds.into_iter().map(Into::into).collect(),
            },
            ommx::PortableCapabilityMismatch::ObjectiveDegreeExceeded {
                actual_degree,
                limit,
            } => Self::ObjectiveDegreeExceeded {
                actual_degree: actual_degree.into_inner(),
                limit: DegreeLimit(limit),
            },
            ommx::PortableCapabilityMismatch::UnsupportedRegularConstraintRelation {
                relation,
                constraint_ids,
                supported_relations,
            } => Self::UnsupportedRegularConstraintRelation {
                relation: relation.into(),
                constraint_ids: constraint_ids
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
                supported_relations: supported_relations.into_iter().map(Into::into).collect(),
            },
            ommx::PortableCapabilityMismatch::RegularConstraintDegreeExceeded {
                relation,
                actual_degrees,
                limit,
            } => Self::RegularConstraintDegreeExceeded {
                relation: relation.into(),
                actual_degrees: actual_degrees
                    .into_iter()
                    .map(|(id, degree)| (id.into_inner(), degree.into_inner()))
                    .collect(),
                limit: DegreeLimit(limit),
            },
            ommx::PortableCapabilityMismatch::UnsupportedIndicatorConstraints {
                constraint_ids,
            } => Self::UnsupportedIndicatorConstraints {
                constraint_ids: constraint_ids
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
            },
            ommx::PortableCapabilityMismatch::UnsupportedIndicatorConstraintRelation {
                relation,
                constraint_ids,
                supported_relations,
            } => Self::UnsupportedIndicatorConstraintRelation {
                relation: relation.into(),
                constraint_ids: constraint_ids
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
                supported_relations: supported_relations.into_iter().map(Into::into).collect(),
            },
            ommx::PortableCapabilityMismatch::IndicatorBodyDegreeExceeded {
                relation,
                actual_degrees,
                limit,
            } => Self::IndicatorBodyDegreeExceeded {
                relation: relation.into(),
                actual_degrees: actual_degrees
                    .into_iter()
                    .map(|(id, degree)| (id.into_inner(), degree.into_inner()))
                    .collect(),
                limit: DegreeLimit(limit),
            },
            ommx::PortableCapabilityMismatch::UnsupportedOneHotConstraints { constraint_ids } => {
                Self::UnsupportedOneHotConstraints {
                    constraint_ids: constraint_ids
                        .into_iter()
                        .map(|id| id.into_inner())
                        .collect(),
                }
            }
            ommx::PortableCapabilityMismatch::UnsupportedSos1Constraints { constraint_ids } => {
                Self::UnsupportedSos1Constraints {
                    constraint_ids: constraint_ids
                        .into_iter()
                        .map(|id| id.into_inner())
                        .collect(),
                }
            }
            ommx::PortableCapabilityMismatch::UnsupportedSense {
                sense,
                supported_senses,
            } => Self::UnsupportedSense {
                sense: sense.into(),
                supported_senses: supported_senses.into_iter().map(Into::into).collect(),
            },
            other => Self::Unknown {
                message: other.to_string(),
            },
        }
    }
}

/// Portable compatibility result for one coherent profile.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCompatibilityReport(pub ommx::ProfileCompatibilityReport);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ProfileCompatibilityReport {
    #[getter]
    pub fn profile_name(&self) -> &str {
        self.0.profile_name()
    }

    #[getter]
    pub fn mismatches(&self) -> Vec<PortableCapabilityMismatch> {
        self.0
            .mismatches()
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }

    #[getter]
    pub fn compatible(&self) -> bool {
        self.0.is_compatible()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ProfileCompatibilityReport(profile_name={:?}, compatible={})",
            self.profile_name(),
            self.compatible()
        )
    }
}

/// Side-effect-free portable compatibility report.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableCompatibilityReport(pub ommx::PortableCompatibilityReport);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PortableCompatibilityReport {
    #[getter]
    pub fn profiles(&self) -> Vec<ProfileCompatibilityReport> {
        self.0
            .profiles()
            .iter()
            .cloned()
            .map(ProfileCompatibilityReport)
            .collect()
    }

    #[getter]
    pub fn compatible(&self) -> bool {
        self.0.is_compatible()
    }

    #[getter]
    pub fn matching_profiles(&self) -> Vec<String> {
        self.0.matching_profiles().map(str::to_string).collect()
    }

    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "PortableCompatibilityReport(compatible={}, matching_profiles={:?})",
            self.compatible(),
            self.matching_profiles()
        )
    }
}
