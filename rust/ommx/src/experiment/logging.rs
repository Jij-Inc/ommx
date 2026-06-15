//! Shared attachment logging APIs for experiment and run handles.

use crate::artifact::media_types;
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::{collections::HashMap, path::Path};

use super::attachment::{
    encode_instance_layer, encode_json, encode_parametric_instance_layer, encode_sample_set_layer,
    encode_solution_layer, json_media_type,
};

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
    fn log_file(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
    ) -> Result<()>;

    /// Attach a JSON-serialisable value.
    fn log_json(self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, value)?;
        self.log_attachment(name, json_media_type(), bytes, HashMap::new())
    }

    /// Attach an [`Instance`].
    fn log_instance(self, name: &str, instance: &Instance) -> Result<()> {
        let (bytes, annotations) = encode_instance_layer(instance);
        self.log_attachment(name, media_types::v1_instance(), bytes, annotations)
    }

    /// Attach a [`ParametricInstance`].
    fn log_parametric_instance(self, name: &str, pi: &ParametricInstance) -> Result<()> {
        let (bytes, annotations) = encode_parametric_instance_layer(pi);
        self.log_attachment(
            name,
            media_types::v1_parametric_instance(),
            bytes,
            annotations,
        )
    }

    /// Attach a [`Solution`].
    fn log_solution(self, name: &str, solution: &Solution) -> Result<()> {
        let (bytes, annotations) = encode_solution_layer(solution);
        self.log_attachment(name, media_types::v1_solution(), bytes, annotations)
    }

    /// Attach a [`SampleSet`].
    fn log_sample_set(self, name: &str, sample_set: &SampleSet) -> Result<()> {
        let (bytes, annotations) = encode_sample_set_layer(sample_set);
        self.log_attachment(name, media_types::v1_sample_set(), bytes, annotations)
    }
}
