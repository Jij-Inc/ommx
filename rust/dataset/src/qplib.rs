use anyhow::{anyhow, Context, Result};
use ommx::{
    artifact::{LocalArtifact, LocalArtifactBuilder},
    ocipkg::ImageName,
};
use std::{fs, path::Path};
use url::Url;
use zip::ZipArchive;

pub fn package(path: &Path) -> Result<()> {
    tracing::info!("Input Archive: {}", path.display());
    let f = fs::File::open(path).with_context(|| format!("File not found: {path:?}"))?;
    let mut ar = ZipArchive::new(f).with_context(|| format!("Not a ZIP archive: {path:?}"))?;

    // Load CSV metadata for validation
    let csv_annotations = ommx::dataset::qplib::instance_annotations();
    tracing::info!(
        "Loaded {} QPLIB metadata entries from CSV",
        csv_annotations.len()
    );

    let source_url = Url::parse("https://github.com/Jij-Inc/ommx")?;

    // Input archive is expected to contain `*.qplib` files.
    for i in 0..ar.len() {
        let file = ar.by_index(i)?;
        let file_name = file.name().to_string();
        let Some(name_with_suffix) = file_name.strip_suffix(".qplib").map(str::to_string) else {
            continue;
        };

        // Extract just the filename (e.g., "QPLIB_3877" from "qplib/html/qplib/QPLIB_3877")
        let name = name_with_suffix
            .split('/')
            .next_back()
            .ok_or_else(|| anyhow!("Invalid file path: {}", file_name))?
            .to_string();

        // Extract numeric tag from name (e.g., "3877" from "QPLIB_3877")
        let tag = name
            .strip_prefix("QPLIB_")
            .ok_or_else(|| anyhow!("Expected QPLIB_ prefix in filename: {}", name))?;

        let image_name = match ImageName::parse(&format!("ghcr.io/jij-inc/ommx/v3/qplib:{tag}")) {
            Ok(name) => name,
            Err(err) => {
                tracing::warn!("Skip: invalid image name for '{name}': {err}");
                continue;
            }
        };
        if LocalArtifact::try_open(image_name.clone())?.is_some() {
            tracing::info!("Skip: {image_name} already in the v3 local registry");
            continue;
        }

        tracing::info!("Loading: {name}");

        let instance = match ommx::qplib::parse(file) {
            Ok(instance) => instance,
            Err(err) => {
                tracing::error!("Skip: Failed to parse '{name}': {err}");
                continue;
            }
        };

        // Get CSV metadata for this instance, or create basic annotations
        let mut annotations = csv_annotations.get(tag).cloned().unwrap_or_else(|| {
            tracing::warn!("No CSV metadata found for instance '{name}', using basic annotations");
            let mut ann = ommx::artifact::InstanceAnnotations::default();
            ann.set_title(name.clone());
            ann.set_dataset("QPLIB".to_string());
            ann
        });

        annotations.set_created_now();

        // Override variables and constraints with actual parsed values
        // QPLIB and OMMX may count constraints differently (e.g., l <= f(x) <= u)
        let nvars = instance.decision_variables().len();
        let ncons = instance.constraints().len();
        annotations.set_variables(nvars);
        annotations.set_constraints(ncons);

        tracing::info!(
            "Packaged '{name}': {} variables, {} constraints",
            nvars,
            ncons
        );

        let mut builder = LocalArtifactBuilder::new(image_name);
        builder.add_source(&source_url);
        builder.add_instance(instance.into(), annotations)?;
        let _artifact = builder.build()?;
        // Do not push here. Use `ommx push` command to upload the artifacts.
    }
    Ok(())
}
