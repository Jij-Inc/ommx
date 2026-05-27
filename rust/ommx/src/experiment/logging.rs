//! Shared attachment logging APIs for experiment and run handles.

use super::{Experiment, ExperimentDyn, Run, RunDyn};
use crate::artifact::media_types;
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;

use super::attachment::{encode_json, json_media_type};

/// A handle that can log attachment payloads into an Experiment space.
///
/// The concrete attachment space depends on the implementor: an
/// [`Experiment`] logs into the experiment-wide space, while a [`Run`]
/// logs into that run's space. The typed `log_*` helpers share the same
/// media-type mapping across both static and dynamic handles.
pub trait AttachmentLogger {
    /// Attach arbitrary bytes with an explicit OCI media type.
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()>;

    /// Attach a JSON-serialisable value.
    fn log_json(self, name: &str, value: impl serde::Serialize) -> Result<()>
    where
        Self: Sized,
    {
        let bytes = encode_json(name, value)?;
        self.log_attachment(name, json_media_type(), bytes)
    }

    /// Attach an [`Instance`].
    fn log_instance(self, name: &str, instance: &Instance) -> Result<()>
    where
        Self: Sized,
    {
        self.log_attachment(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Attach a [`ParametricInstance`].
    fn log_parametric_instance(self, name: &str, pi: &ParametricInstance) -> Result<()>
    where
        Self: Sized,
    {
        self.log_attachment(name, media_types::v1_parametric_instance(), pi.to_bytes())
    }

    /// Attach a [`Solution`].
    fn log_solution(self, name: &str, solution: &Solution) -> Result<()>
    where
        Self: Sized,
    {
        self.log_attachment(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Attach a [`SampleSet`].
    fn log_sample_set(self, name: &str, sample_set: &SampleSet) -> Result<()>
    where
        Self: Sized,
    {
        self.log_attachment(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }
}

impl<'a, 'reg> AttachmentLogger for &'a Experiment<'reg> {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_attachment(name, media_type, bytes.as_ref())
    }
}

impl<'a, 'exp, 'reg> AttachmentLogger for &'a mut Run<'exp, 'reg> {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_attachment(name, media_type, bytes.as_ref())
    }
}

impl<'a> AttachmentLogger for &'a ExperimentDyn {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_attachment(name, media_type, bytes.as_ref())
    }
}

impl<'a> AttachmentLogger for &'a mut RunDyn {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_attachment(name, media_type, bytes.as_ref())
    }
}
