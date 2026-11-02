//! Flat, nullable tables stored in Parquet artifact layers.
//!
//! This codec uses the native Rust Parquet implementation without its Arrow
//! feature. It supports boolean, integer, floating-point, UTF-8, binary, date
//! and timestamp columns.
//! Nested, decimal, INT96 and other unsupported schemas are rejected explicitly.
//! Metadata is preserved as strings; interpretation belongs to the consumer.
//!
//! Responsibilities are separated at the table boundary:
//! - [Table] validates flat columns and maps their types to Parquet.
//! - The parquet crate owns file encoding, pages and compression.
//! - Consumers such as the Python binding own dtype/index/category metadata.
//! - Artifact operations below own layer media types and registry-backed storage.
//!
//! ```
//! use ommx::artifact::parquet::{Column, ColumnData, Table};
//!
//! let table = Table::new(
//!     vec![Column {
//!         name: "objective".into(),
//!         data: ColumnData::Float64(vec![Some(1.5), None]),
//!     }],
//!     Default::default(),
//! )?;
//! let decoded = Table::from_parquet(table.to_parquet()?)?;
//! assert_eq!(decoded, table);
//! # Ok::<_, ommx::Error>(())
//! ```

mod table;

pub use table::{Column, ColumnData, Table};

use crate::{
    artifact::{local_registry::StoredDescriptor, media_types, ArtifactDraft, LocalArtifact},
    Result,
};

impl<'reg> ArtifactDraft<'reg> {
    /// Encode and add a flat Parquet table as an artifact layer.
    ///
    /// Encoding finishes before the registry or draft is changed.
    pub fn add_parquet(
        &mut self,
        table: &Table,
        annotations: std::collections::HashMap<String, String>,
    ) -> Result<StoredDescriptor<'reg>> {
        let bytes = table.to_parquet()?;
        self.add_layer_bytes(media_types::PARQUET.into(), bytes, annotations)
    }
}

impl LocalArtifact<'_> {
    /// Read a flat Parquet table from a stored artifact layer.
    pub fn get_parquet(&self, descriptor: &StoredDescriptor<'_>) -> Result<Table> {
        descriptor.ensure_media_type(&media_types::PARQUET.into())?;
        Table::from_parquet(self.get_blob(descriptor)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn table(data: ColumnData) -> Table {
        Table::new(
            vec![Column {
                name: "value".into(),
                data,
            }],
            BTreeMap::from([("consumer".into(), "opaque metadata".into())]),
        )
        .unwrap()
    }

    #[test]
    fn artifact_roundtrip_and_descriptor_ownership() {
        use crate::artifact::local_registry::LocalRegistry;
        let root = tempfile::tempdir().unwrap();
        let registry = LocalRegistry::open(root.path()).unwrap();
        let mut draft = ArtifactDraft::new_anonymous_in_registry(&registry).unwrap();
        let expected = table(ColumnData::Int64(vec![Some(42), None]));
        let descriptor = draft
            .add_parquet(&expected, [("source".into(), "results".into())].into())
            .unwrap();
        let wrong_media_type = draft
            .add_layer_bytes(
                "application/octet-stream".into(),
                expected.to_parquet().unwrap(),
                Default::default(),
            )
            .unwrap();
        let artifact = draft.commit().unwrap();
        assert_eq!(descriptor.media_type().as_ref(), media_types::PARQUET);
        assert_eq!(
            descriptor.annotations().as_ref().unwrap()["source"],
            "results"
        );
        assert_eq!(artifact.get_parquet(&descriptor).unwrap(), expected);
        assert!(artifact.get_parquet(&wrong_media_type).is_err());

        let root2 = tempfile::tempdir().unwrap();
        let registry2 = LocalRegistry::open(root2.path()).unwrap();
        let artifact2 = ArtifactDraft::new_anonymous_in_registry(&registry2)
            .unwrap()
            .commit()
            .unwrap();
        assert!(artifact2.get_parquet(&descriptor).is_err());
    }
}
