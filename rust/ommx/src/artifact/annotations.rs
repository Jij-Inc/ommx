use anyhow::Result;
use ocipkg::Digest;
use std::collections::HashMap;

/// Annotations for [`application/org.ommx.v1.instance`][crate::artifact::media_type::v1_instance]
#[non_exhaustive]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InstanceAnnotations {}

impl From<InstanceAnnotations> for HashMap<String, String> {
    fn from(_: InstanceAnnotations) -> Self {
        HashMap::new()
    }
}

impl TryFrom<HashMap<String, String>> for InstanceAnnotations {
    type Error = anyhow::Error;
    fn try_from(_: HashMap<String, String>) -> Result<Self> {
        Ok(Self {})
    }
}

/// Annotations for [`application/org.ommx.v1.solution`][crate::artifact::media_type::v1_solution]
#[non_exhaustive]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SolutionAnnotations {
    /// A reference to the instance of the solution stored with the key `org.ommx.v1.solution.instance`
    pub instance: Option<Digest>,
    /// A reference to the solver information which generated the solution stored with the key `org.ommx.v1.solution.solver`
    pub solver: Option<Digest>,
    /// JSON encoded parameters used to generate the solution stored with the key `org.ommx.v1.solution.parameters`
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

impl TryFrom<HashMap<String, String>> for SolutionAnnotations {
    type Error = anyhow::Error;
    fn try_from(mut map: HashMap<String, String>) -> Result<Self> {
        Ok(Self {
            instance: map
                .remove("org.ommx.v1.solution.instance")
                .map(|s| Digest::new(&s))
                .transpose()?,
            solver: map
                .remove("org.ommx.v1.solution.solver")
                .map(|s| Digest::new(&s))
                .transpose()?,
            parameters: map.remove("org.ommx.v1.solution.parameters"),
        })
    }
}
