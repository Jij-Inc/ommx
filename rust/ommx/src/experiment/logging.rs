//! Shared attachment logging APIs for experiment and run handles.

use crate::artifact::local_registry::{registry::attachment_storage, LocalRegistry};
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{ensure, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::{collections::HashMap, io::Read, path::Path};

use super::attachment::{
    encode_json, json_media_type, open_file_attachment, prepare_attachment_storage,
    AttachmentTable, Compression,
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

    /// Attach arbitrary bytes and optionally compress their stored OCI layer.
    fn log_attachment_compressed(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
        compression: Compression,
    ) -> Result<()>;

    /// Stream an attachment from a reader without buffering the full payload.
    fn log_attachment_from_reader(
        self,
        name: &str,
        media_type: MediaType,
        reader: impl Read,
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

    /// Attach a filesystem file and optionally compress its stored OCI layer.
    fn log_file_compressed(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
        compression: Compression,
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
        self.log_attachment_compressed(name, media_type, bytes, annotations, Compression::None)
    }

    fn log_attachment_compressed(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
        compression: Compression,
    ) -> Result<()> {
        log_attachment_reader(
            self,
            name,
            media_type,
            bytes.as_ref(),
            annotations,
            None,
            compression,
        )
    }

    fn log_attachment_from_reader(
        self,
        name: &str,
        media_type: MediaType,
        reader: impl Read,
        annotations: HashMap<String, String>,
    ) -> Result<()> {
        log_attachment_reader(
            self,
            name,
            media_type,
            reader,
            annotations,
            None,
            Compression::None,
        )
    }

    fn log_file(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
    ) -> Result<()> {
        self.log_file_compressed(name, path, media_type, filename, Compression::None)
    }

    fn log_file_compressed(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
        compression: Compression,
    ) -> Result<()> {
        let (media_type, file, filename) = open_file_attachment(path, media_type, filename)?;
        log_attachment_reader(
            self,
            name,
            media_type,
            file,
            HashMap::new(),
            Some(filename),
            compression,
        )
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

pub trait AttachmentLoggerStorage: Sized {
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

fn log_attachment_reader<T: AttachmentLoggerStorage>(
    mut logger: T,
    name: &str,
    media_type: MediaType,
    reader: impl Read,
    annotations: HashMap<String, String>,
    filename: Option<String>,
    compression: Compression,
) -> Result<()> {
    ensure_attachment_name_available(&mut logger, name)?;
    let (stored_media_type, annotations) =
        prepare_attachment_storage(compression, media_type, annotations)?;
    let descriptor = AttachmentLoggerStorage::with_local_registry(&logger, |registry| {
        let descriptor = match compression {
            Compression::None => attachment_storage::store_layer_reader(
                registry,
                stored_media_type,
                reader,
                annotations,
            )?,
            Compression::Zstd => {
                let encoder = zstd::stream::read::Encoder::new(reader, 0)?;
                attachment_storage::store_layer_reader(
                    registry,
                    stored_media_type,
                    encoder,
                    annotations,
                )?
            }
        };
        Ok(Descriptor::from(descriptor))
    })?;
    let descriptor = logger.descriptor_for_attachment_table(descriptor)?;
    logger.with_attachment_table(|attachments| {
        attachments.insert(name.to_string(), descriptor, filename)
    })
}
