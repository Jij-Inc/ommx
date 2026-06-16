//! Shared attachment logging APIs for experiment and run handles.

use crate::artifact::local_registry::LocalRegistry;
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{ensure, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::{collections::HashMap, path::Path};

use super::attachment::{encode_json, json_media_type, read_file_attachment, AttachmentTable};

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
    fn log_instance(self, name: &str, instance: &Instance) -> Result<()>;

    /// Attach a [`ParametricInstance`].
    fn log_parametric_instance(self, name: &str, pi: &ParametricInstance) -> Result<()>;

    /// Attach a [`Solution`].
    fn log_solution(self, name: &str, solution: &Solution) -> Result<()>;

    /// Attach a [`SampleSet`].
    fn log_sample_set(self, name: &str, sample_set: &SampleSet) -> Result<()>;
}

impl<T> AttachmentLogger for T
where
    T: AttachmentLoggerStorage,
{
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
    ) -> Result<()> {
        let mut logger = self;
        ensure_attachment_name_available(&mut logger, name)?;
        let bytes = bytes.as_ref();
        let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
            let descriptor = registry.store_layer_blob(media_type, bytes, annotations)?;
            Ok(Descriptor::from(descriptor))
        })?;
        let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
        logger.with_attachment_table(|attachments| {
            attachments.insert(name.to_string(), descriptor, None)
        })
    }

    fn log_file(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
    ) -> Result<()> {
        let mut logger = self;
        ensure_attachment_name_available(&mut logger, name)?;
        let (media_type, bytes, filename) = read_file_attachment(path, media_type, filename)?;
        let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
            let descriptor =
                registry.store_layer_blob(media_type, bytes.as_ref(), HashMap::new())?;
            Ok(Descriptor::from(descriptor))
        })?;
        let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
        logger.with_attachment_table(|attachments| {
            attachments.insert(name.to_string(), descriptor, Some(filename))
        })
    }

    fn log_instance(self, name: &str, instance: &Instance) -> Result<()> {
        let mut logger = self;
        ensure_attachment_name_available(&mut logger, name)?;
        let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
            let descriptor = registry.store_instance_layer(instance)?;
            Ok(Descriptor::from(descriptor))
        })?;
        let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
        logger.with_attachment_table(|attachments| {
            attachments.insert(name.to_string(), descriptor, None)
        })
    }

    fn log_parametric_instance(self, name: &str, pi: &ParametricInstance) -> Result<()> {
        let mut logger = self;
        ensure_attachment_name_available(&mut logger, name)?;
        let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
            let descriptor = registry.store_parametric_instance_layer(pi)?;
            Ok(Descriptor::from(descriptor))
        })?;
        let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
        logger.with_attachment_table(|attachments| {
            attachments.insert(name.to_string(), descriptor, None)
        })
    }

    fn log_solution(self, name: &str, solution: &Solution) -> Result<()> {
        let mut logger = self;
        ensure_attachment_name_available(&mut logger, name)?;
        let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
            let descriptor = registry.store_solution_layer(solution)?;
            Ok(Descriptor::from(descriptor))
        })?;
        let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
        logger.with_attachment_table(|attachments| {
            attachments.insert(name.to_string(), descriptor, None)
        })
    }

    fn log_sample_set(self, name: &str, sample_set: &SampleSet) -> Result<()> {
        let mut logger = self;
        ensure_attachment_name_available(&mut logger, name)?;
        let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
            let descriptor = registry.store_sample_set_layer(sample_set)?;
            Ok(Descriptor::from(descriptor))
        })?;
        let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
        logger.with_attachment_table(|attachments| {
            attachments.insert(name.to_string(), descriptor, None)
        })
    }
}

pub(super) trait AttachmentLoggerStorage: Sized {
    type Descriptor;

    fn with_local_registry<R>(&self, f: impl FnOnce(&LocalRegistry) -> Result<R>) -> Result<R>;

    fn with_attachment_table<R>(
        &mut self,
        f: impl FnOnce(&mut AttachmentTable<Self::Descriptor>) -> Result<R>,
    ) -> Result<R>;

    fn descriptor_for_attachment_table(&self, descriptor: Descriptor) -> Result<Self::Descriptor>;
}

fn ensure_attachment_name_available<T: AttachmentLoggerStorage>(
    logger: &mut T,
    name: &str,
) -> Result<()> {
    logger.with_attachment_table(|attachments| {
        ensure!(
            !attachments.contains_key(name),
            "Attachment `{name}` already exists"
        );
        Ok(())
    })
}
