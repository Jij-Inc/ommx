use anyhow::{Context, Result};
use derive_more::{Deref, From, Into};
use ocipkg::Digest;
use serde::{Deserialize, Serialize};
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
#[derive(Debug, Default, Clone, PartialEq, From, Deref, Into)]
pub struct SolutionAnnotations(HashMap<String, String>);

impl SolutionAnnotations {
    /// Set the value of [Self::instance]
    pub fn set_instance(&mut self, digest: Digest) {
        self.0.insert(
            "org.ommx.v1.solution.instance".to_string(),
            digest.to_string(),
        );
    }

    /// A reference to the instance of the solution stored with the key `org.ommx.v1.solution.instance`
    pub fn instance(&self) -> Result<Digest> {
        let digest = self.0.get("org.ommx.v1.solution.instance").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.instance`",
        )?;
        Ok(Digest::new(digest)?)
    }

    /// Set the value of [Self::solver]
    pub fn set_solver(&mut self, digest: Digest) {
        self.0.insert(
            "org.ommx.v1.solution.solver".to_string(),
            digest.to_string(),
        );
    }

    /// A reference to the solver information which generated the solution stored with the key `org.ommx.v1.solution.solver`
    pub fn solver(&self) -> Result<Digest> {
        let digest = self.0.get("org.ommx.v1.solution.solver").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.solver`",
        )?;
        Ok(Digest::new(digest)?)
    }

    /// Set the value of [Self::parameters]
    pub fn set_parameters(&mut self, parameters: impl Serialize) -> Result<()> {
        self.0.insert(
            "org.ommx.v1.solution.parameters".to_string(),
            serde_json::to_string(&parameters)?,
        );
        Ok(())
    }

    /// Solver parameters used to generate the solution as a JSON with the key `org.ommx.v1.solution.parameters`
    pub fn parameters<'s: 'de, 'de, P: Deserialize<'de>>(&'s self) -> Result<P> {
        Ok(serde_json::from_str(
            self.0.get("org.ommx.v1.solution.parameters").context(
                "Annotation does not have the entry with the key `org.ommx.v1.solution.parameters`",
            )?,
        )?)
    }

    /// Set other annotations. The key may not start with `org.ommx.v1.solution`, but must start with other valid prefix.
    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}
