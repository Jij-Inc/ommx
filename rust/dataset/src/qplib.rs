use anyhow::{Context, Result};
use ommx::{artifact::Builder, Kind};
use serde::Serialize;
use std::{fs, path::Path};
use zip::ZipArchive;

#[derive(Default, Serialize)]
struct ReportRow {
    name: String,
    image: String,
    status: &'static str,
    detail: String,
    variables: Option<usize>,
    binaries: Option<usize>,
    integers: Option<usize>,
    continuous: Option<usize>,
    constraints: Option<usize>,
    instance_digest: String,
}

pub fn package(path: &Path, report: Option<&Path>) -> Result<()> {
    let annotation_dict = ommx::dataset::qplib::instance_annotations();
    log::info!("Input Archive: {}", path.display());
    let f = fs::File::open(path).with_context(|| format!("File not found: {path:?}"))?;
    let mut ar = ZipArchive::new(f).with_context(|| format!("Not a ZIP archive: {path:?}"))?;
    let mut report = report.map(csv::Writer::from_path).transpose()?;
    let mut packaged = 0;
    let mut failed = 0;

    for i in 0..ar.len() {
        let file = ar.by_index(i)?;
        let Some(name) = file
            .name()
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".qplib"))
            .map(str::to_string)
        else {
            continue;
        };
        let mut row = ReportRow {
            name: name.clone(),
            ..Default::default()
        };
        let result = (|| -> Result<()> {
            let tag = name
                .strip_prefix("QPLIB_")
                .with_context(|| format!("Expected QPLIB_ prefix in filename: {name}"))?;
            row.image = format!("ghcr.io/jij-inc/ommx/v2.8/qplib:{tag}");
            let annotations = annotation_dict
                .get(tag)
                .with_context(|| format!("No metadata found for '{name}'"))?;
            log::info!("Loading: {name}");
            let instance = ommx::qplib::parse(file)?;
            row.variables = Some(instance.decision_variables().len());
            row.constraints = Some(instance.constraints().len());
            let count = |kind| {
                instance
                    .decision_variables()
                    .values()
                    .filter(|variable| variable.kind() == kind)
                    .count()
            };
            row.binaries = Some(count(Kind::Binary));
            row.integers = Some(count(Kind::Integer));
            row.continuous = Some(count(Kind::Continuous));
            let expected_variables: usize = annotations
                .get("org.ommx.qplib.nvars")
                .context("Missing QPLIB variable count")?
                .parse()?;
            anyhow::ensure!(
                instance.decision_variables().len() == expected_variables,
                "Variable count mismatch: expected {expected_variables}, found {}",
                instance.decision_variables().len()
            );

            // Validate before creating the OCI directory so failed inputs do
            // not leave incomplete Artifacts in the registry.
            let mut builder = Builder::for_github("Jij-Inc", "ommx", "v2.8/qplib", tag)?;
            let mut annotations = annotations.clone();
            annotations.set_created_now();
            annotations.set_variables(instance.decision_variables().len());
            annotations.set_constraints(instance.constraints().len());
            annotations.set_other(
                "org.ommx.qplib.parser_version".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            );
            builder.add_instance(instance.into(), annotations)?;
            let mut artifact = builder.build()?;
            row.instance_digest = artifact.get_manifest()?.layers()[0].digest().to_string();
            Ok(())
        })();
        match result {
            Ok(()) => {
                row.status = "packaged";
                packaged += 1;
                log::info!("Packaged: {}", row.image);
            }
            Err(err) => {
                row.status = "failed";
                row.detail = format!("{err:#}");
                failed += 1;
                log::error!("Failed: {name}: {}", row.detail);
            }
        }
        if let Some(report) = &mut report {
            report.serialize(&row)?;
            report.flush()?;
        }
    }
    log::info!("QPLIB: {packaged} packaged, {failed} failed");
    // Do not push here. Review the report before publishing the OCI layouts.
    Ok(())
}
