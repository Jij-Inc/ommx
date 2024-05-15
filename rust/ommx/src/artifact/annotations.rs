use ocipkg::Digest;
use std::collections::HashMap;

#[non_exhaustive]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InstanceAnnotations {}

impl From<InstanceAnnotations> for HashMap<String, String> {
    fn from(_: InstanceAnnotations) -> Self {
        HashMap::new()
    }
}

#[non_exhaustive]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SolutionAnnotations {
    /// A reference to the instance of the solution
    pub instance: Option<Digest>,
    /// A reference to the solver information which generated the solution
    pub solver: Option<Digest>,
    /// JSON encoded parameters used to generate the solution
    pub parameters: Option<String>,
}

impl From<SolutionAnnotations> for HashMap<String, String> {
    fn from(annotations: SolutionAnnotations) -> Self {
        let mut out = HashMap::new();
        if let Some(instance) = annotations.instance {
            out.insert(
                "org.ommx.v1.solution.instance".to_string(),
                instance.to_string(),
            );
        }
        if let Some(solver) = annotations.solver {
            out.insert(
                "org.ommx.v1.solution.solver".to_string(),
                solver.to_string(),
            );
        }
        if let Some(parameters) = annotations.parameters {
            out.insert("org.ommx.v1.solution.parameters".to_string(), parameters);
        }
        out
    }
}
