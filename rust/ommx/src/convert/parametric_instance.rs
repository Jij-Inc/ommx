use crate::v1::{Instance, Parameters, ParametricInstance, State};
use anyhow::Result;

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            parameters: _, // Drop previous parameters
        }: Instance,
    ) -> Self {
        Self {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            parameters: Default::default(),
        }
    }
}

impl From<State> for Parameters {
    fn from(State { entries }: State) -> Self {
        Self { entries }
    }
}

impl From<Parameters> for State {
    fn from(Parameters { entries }: Parameters) -> Self {
        Self { entries }
    }
}

impl ParametricInstance {
    /// Create a new [Instance] with the given parameters.
    pub fn with_parameters(&self, _parameters: Parameters) -> Result<Instance> {
        todo!()
    }
}
