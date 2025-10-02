use anyhow::{anyhow, Context, Result};
use ommx::artifact::Builder;
use std::{fs, path::Path};
use zip::ZipArchive;

pub fn package(path: &Path) -> Result<()> {
    log::info!("Input Archive: {}", path.display());
    let f = fs::File::open(path).with_context(|| format!("File not found: {path:?}"))?;
    let mut ar = ZipArchive::new(f).with_context(|| format!("Not a ZIP archive: {path:?}"))?;

    // Load CSV metadata for validation
    let csv_annotations = ommx::dataset::qplib::instance_annotations();
    log::info!(
        "Loaded {} QPLIB metadata entries from CSV",
        csv_annotations.len()
    );

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
            .last()
            .ok_or_else(|| anyhow!("Invalid file path: {}", file_name))?
            .to_string();

        // Extract numeric tag from name (e.g., "3877" from "QPLIB_3877")
        let tag = name
            .strip_prefix("QPLIB_")
            .ok_or_else(|| anyhow!("Expected QPLIB_ prefix in filename: {}", name))?;

        let Ok(mut builder) = Builder::for_github("Jij-Inc", "ommx", "qplib", tag) else {
            log::warn!("Skip: container already exists for '{name}'");
            continue;
        };

        log::info!("Loading: {name}");

        let instance = match ommx::qplib::parse(file) {
            Ok(instance) => instance,
            Err(err) => {
                log::error!("Skip: Failed to parse '{name}': {err}");
                continue;
            }
        };

        // Get CSV metadata for this instance
        let csv_annotation = match csv_annotations.get(&name) {
            Some(ann) => ann,
            None => {
                log::warn!("No CSV metadata found for instance '{name}', using basic annotations");
                // Create basic annotations if CSV metadata is missing
                let mut annotations = ommx::artifact::InstanceAnnotations::default();
                annotations.set_title(name.clone());
                annotations.set_created_now();
                annotations.set_dataset("QPLIB".to_string());
                annotations.set_variables(instance.decision_variables().len());
                annotations.set_constraints(instance.constraints().len());

                builder.add_instance(instance.into(), annotations)?;
                let _artifact = builder.build()?;
                continue;
            }
        };

        // Use CSV annotations enriched with all metadata
        let mut annotations = csv_annotation.clone();
        annotations.set_created_now();

        // Override variables and constraints with actual parsed values
        // QPLIB and OMMX may count constraints differently (e.g., l <= f(x) <= u)
        let nvars = instance.decision_variables().len();
        let ncons = instance.constraints().len();
        annotations.set_variables(nvars);
        annotations.set_constraints(ncons);

        log::info!(
            "Packaged '{name}': {} variables, {} constraints",
            nvars,
            ncons
        );

        builder.add_instance(instance.into(), annotations)?;
        let _artifact = builder.build()?;
        // Do not push here. Use `ommx push` command to upload the artifacts.
    }
    Ok(())
}
