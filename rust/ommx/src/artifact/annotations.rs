use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use derive_more::{Deref, From, Into};
use ocipkg::{oci_spec::image::Descriptor, Digest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Annotations for [`application/org.ommx.v1.instance`][crate::artifact::media_types::v1_instance]
#[derive(Debug, Default, Clone, PartialEq, From, Deref, Into)]
pub struct InstanceAnnotations(HashMap<String, String>);

impl InstanceAnnotations {
    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self(desc.annotations().as_ref().cloned().unwrap_or_default())
    }

    pub fn set_title(&mut self, title: String) {
        self.0
            .insert("org.ommx.v1.instance.title".to_string(), title);
    }

    pub fn title(&self) -> Result<&String> {
        self.0
            .get("org.ommx.v1.instance.title")
            .context("Annotation does not have the entry with the key `org.ommx.v1.instance.title`")
    }

    pub fn set_created(&mut self, created: DateTime<Local>) {
        self.0.insert(
            "org.ommx.v1.instance.created".to_string(),
            created.to_rfc3339(),
        );
    }

    pub fn created(&self) -> Result<DateTime<Local>> {
        let created = self.0.get("org.ommx.v1.instance.created").context(
            "Annotation does not have the entry with the key `org.ommx.v1.instance.created`",
        )?;
        Ok(DateTime::parse_from_rfc3339(created)?.with_timezone(&Local))
    }

    /// Set other annotations. The key may not start with `org.ommx.v1.`, but must a valid reverse domain name.
    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}

/// Annotations for [`application/org.ommx.v1.solution`][crate::artifact::media_types::v1_solution]
#[derive(Debug, Default, Clone, PartialEq, From, Deref, Into)]
pub struct SolutionAnnotations(HashMap<String, String>);

impl SolutionAnnotations {
    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self(desc.annotations().as_ref().cloned().unwrap_or_default())
    }

    pub fn set_start(&mut self, start: DateTime<Local>) {
        self.0
            .insert("org.ommx.v1.solution.start".to_string(), start.to_rfc3339());
    }

    pub fn start(&self) -> Result<DateTime<Local>> {
        let start = self.0.get("org.ommx.v1.solution.start").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.start`",
        )?;
        Ok(DateTime::parse_from_rfc3339(start)?.with_timezone(&Local))
    }

    pub fn set_end(&mut self, end: DateTime<Local>) {
        self.0
            .insert("org.ommx.v1.solution.end".to_string(), end.to_rfc3339());
    }

    pub fn end(&self) -> Result<DateTime<Local>> {
        let end = self.0.get("org.ommx.v1.solution.end").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.end`",
        )?;
        Ok(DateTime::parse_from_rfc3339(end)?.with_timezone(&Local))
    }

    /// Set `org.ommx.v1.solution.instance`
    pub fn set_instance(&mut self, digest: Digest) {
        self.0.insert(
            "org.ommx.v1.solution.instance".to_string(),
            digest.to_string(),
        );
    }

    /// Get `org.ommx.v1.solution.instance`
    pub fn instance(&self) -> Result<Digest> {
        let digest = self.0.get("org.ommx.v1.solution.instance").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.instance`",
        )?;
        Digest::new(digest)
    }

    /// Set `org.ommx.v1.solution.solver`
    pub fn set_solver(&mut self, digest: Digest) {
        self.0.insert(
            "org.ommx.v1.solution.solver".to_string(),
            digest.to_string(),
        );
    }

    /// Get `org.ommx.v1.solution.solver`
    pub fn solver(&self) -> Result<Digest> {
        let digest = self.0.get("org.ommx.v1.solution.solver").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.solver`",
        )?;
        Digest::new(digest)
    }

    /// Set `org.ommx.v1.solution.parameters`
    pub fn set_parameters(&mut self, parameters: impl Serialize) -> Result<()> {
        self.0.insert(
            "org.ommx.v1.solution.parameters".to_string(),
            serde_json::to_string(&parameters)?,
        );
        Ok(())
    }

    /// Get `org.ommx.v1.solution.parameters`
    pub fn parameters<'s: 'de, 'de, P: Deserialize<'de>>(&'s self) -> Result<P> {
        Ok(serde_json::from_str(
            self.0.get("org.ommx.v1.solution.parameters").context(
                "Annotation does not have the entry with the key `org.ommx.v1.solution.parameters`",
            )?,
        )?)
    }

    /// Set other annotations
    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}
