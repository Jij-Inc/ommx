//! Shared attachment logging APIs for experiment and run handles.

use crate::artifact::{
    media_types, InstanceAnnotations, ParametricInstanceAnnotations, SampleSetAnnotations,
    SolutionAnnotations,
};
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;

use super::attachment::{encode_json, json_media_type, FileAttachment};

/// A handle that can log attachment payloads into an Experiment space.
///
/// The concrete attachment space depends on the implementor: an
/// [`Experiment`](crate::experiment::Experiment) logs into the experiment-wide
/// space, while a [`Run`](crate::experiment::Run) logs into that run's space.
/// The typed `log_*` helpers share the same media-type mapping across both
/// static and dynamic handles.
pub trait AttachmentLogger: Sized {
    /// Attach arbitrary bytes with an explicit OCI media type and layer annotations.
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
    ) -> Result<()>;

    /// Attach an existing filesystem file with export filename metadata.
    fn log_file(self, name: &str, attachment: FileAttachment) -> Result<()>;

    /// Attach a JSON-serialisable value.
    fn log_json(self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, value)?;
        self.log_attachment(name, json_media_type(), bytes, HashMap::new())
    }

    /// Attach an [`Instance`] with its artifact annotations.
    fn log_instance(
        self,
        name: &str,
        instance: &Instance,
        annotations: InstanceAnnotations,
    ) -> Result<()> {
        self.log_attachment(
            name,
            media_types::v1_instance(),
            instance.to_bytes(),
            annotations.into_inner(),
        )
    }

    /// Attach a [`ParametricInstance`] with its artifact annotations.
    fn log_parametric_instance(
        self,
        name: &str,
        pi: &ParametricInstance,
        annotations: ParametricInstanceAnnotations,
    ) -> Result<()> {
        self.log_attachment(
            name,
            media_types::v1_parametric_instance(),
            pi.to_bytes(),
            annotations.into_inner(),
        )
    }

    /// Attach a [`Solution`] with its artifact annotations.
    fn log_solution(
        self,
        name: &str,
        solution: &Solution,
        annotations: SolutionAnnotations,
    ) -> Result<()> {
        self.log_attachment(
            name,
            media_types::v1_solution(),
            solution.to_bytes(),
            annotations.into_inner(),
        )
    }

    /// Attach a [`SampleSet`] with its artifact annotations.
    fn log_sample_set(
        self,
        name: &str,
        sample_set: &SampleSet,
        annotations: SampleSetAnnotations,
    ) -> Result<()> {
        self.log_attachment(
            name,
            media_types::v1_sample_set(),
            sample_set.to_bytes(),
            annotations.into_inner(),
        )
    }
}
