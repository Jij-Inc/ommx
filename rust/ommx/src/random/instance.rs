use crate::{
    random::{arbitrary_constraints, arbitrary_decision_variables, FunctionParameters},
    v1::{
        decision_variable::Kind,
        instance::{Description, Sense},
        Function, Instance,
    },
};
use anyhow::{bail, Result};
use proptest::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceParameters {
    pub num_constraints: usize,
    pub objective: FunctionParameters,
    pub constraint: FunctionParameters,
    pub kinds: Vec<Kind>,
}

impl Kind {
    pub fn possibles() -> Vec<Self> {
        vec![
            Kind::Continuous,
            Kind::Integer,
            Kind::Binary,
            Kind::SemiContinuous,
            Kind::SemiInteger,
        ]
    }
}

impl InstanceParameters {
    /// Example parameter for LP problem, i.e. linear objective and constraints with continuous decision variables.
    pub fn default_lp() -> Self {
        Self {
            num_constraints: 5,
            objective: FunctionParameters {
                num_terms: 5,
                max_degree: 1,
                max_id: 10,
            },
            constraint: FunctionParameters {
                num_terms: 5,
                max_degree: 1,
                max_id: 10,
            },
            kinds: vec![Kind::Continuous],
        }
    }

    /// Example parameter for binary problem
    pub fn default_binary() -> Self {
        Self {
            kinds: vec![Kind::Binary],
            ..Default::default()
        }
    }

    /// Example parameter for binary problem without constraints
    pub fn default_pubo() -> Self {
        Self {
            num_constraints: 0,
            kinds: vec![Kind::Binary],
            ..Default::default()
        }
    }

    /// Example parameter for binary quadratic problem without constraints (QUBO)
    pub fn default_qubo() -> Self {
        Self {
            num_constraints: 0,
            objective: FunctionParameters {
                num_terms: 5,
                max_degree: 2,
                max_id: 10,
            },
            kinds: vec![Kind::Binary],
            ..Default::default()
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.objective.validate()?;
        self.constraint.validate()?;
        if self.kinds.is_empty() {
            bail!("At least one kind of decision variable must be allowed");
        }
        Ok(())
    }

    pub fn smaller(&self) -> BoxedStrategy<Self> {
        (
            0..=self.num_constraints,
            self.objective.smaller(),
            self.constraint.smaller(),
            proptest::sample::subsequence(self.kinds.clone(), 1..=self.kinds.len()),
        )
            .prop_map(
                |(num_constraints, objective, constraint, kinds)| InstanceParameters {
                    objective,
                    constraint,
                    kinds,
                    num_constraints,
                },
            )
            .boxed()
    }
}

impl Default for InstanceParameters {
    fn default() -> Self {
        Self {
            num_constraints: 5,
            objective: FunctionParameters::default(),
            constraint: FunctionParameters::default(),
            kinds: vec![Kind::Continuous, Kind::Integer, Kind::Binary],
        }
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        let InstanceParameters {
            num_constraints,
            objective,
            constraint,
            kinds,
        } = p;

        (
            Function::arbitrary_with(objective),
            arbitrary_constraints(num_constraints, constraint),
        )
            .prop_flat_map(move |(objective, constraints)| {
                let mut used_ids = objective.used_decision_variable_ids();
                for c in &constraints {
                    used_ids.extend(c.function().used_decision_variable_ids());
                }
                let relaxed = if constraints.is_empty() {
                    Just(Vec::new()).boxed()
                } else {
                    let constraint_ids = constraints.iter().map(|c| c.id).collect::<Vec<_>>();
                    proptest::sample::subsequence(constraint_ids, 0..=constraints.len()).boxed()
                };
                (
                    Just(objective),
                    Just(constraints),
                    arbitrary_decision_variables(used_ids, kinds.clone()),
                    Option::<Description>::arbitrary(),
                    Sense::arbitrary(),
                    relaxed,
                    ".{0,3}",
                    proptest::collection::hash_map(".{0,3}", ".{0,3}", 0..=2),
                )
                    .prop_map(
                        |(
                            objective,
                            constraints,
                            decision_variables,
                            description,
                            sense,
                            relaxed,
                            removed_reason,
                            removed_parameters,
                        )| {
                            let mut instance = Instance {
                                objective: Some(objective),
                                constraints,
                                decision_variables,
                                description,
                                sense: sense as i32,
                                ..Default::default()
                            };
                            for i in relaxed {
                                instance
                                    .relax_constraint(
                                        i,
                                        removed_reason.clone(),
                                        removed_parameters.clone(),
                                    )
                                    .unwrap();
                            }
                            instance
                        },
                    )
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        Self::Parameters::default()
            .smaller()
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

impl Arbitrary for Sense {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        prop_oneof![Just(Sense::Minimize), Just(Sense::Maximize)].boxed()
    }
}

impl Arbitrary for Description {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        (
            proptest::option::of(".{0,3}"),
            proptest::option::of(".{0,3}"),
            prop_oneof![Just(Vec::new()), proptest::collection::vec(".*", 1..3)],
            proptest::option::of(".{0,3}"),
        )
            .prop_map(|(name, description, authors, created_by)| Description {
                name,
                description,
                authors,
                created_by,
            })
            .boxed()
    }
}
